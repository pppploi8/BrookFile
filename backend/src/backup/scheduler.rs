use crate::backup::manager::BackupManager;
use chrono::{Utc, NaiveTime, NaiveDate, Datelike, Duration, TimeZone};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use rusqlite::params;
use tokio_util::sync::CancellationToken;

pub struct ScheduleConfig {
    pub cycle: String,
    pub backup_time: serde_json::Value,
}

pub struct ScheduledTask {
    pub config: ScheduleConfig,
    pub next_run_time: chrono::DateTime<chrono::Utc>,
}

pub struct BackupScheduler {
    scheduled_tasks: Arc<RwLock<HashMap<String, ScheduledTask>>>,
    backup_manager: Arc<BackupManager>,
    pool: crate::database::Pool,
    shutdown_token: CancellationToken,
}

impl BackupScheduler {
    pub fn new(backup_manager: Arc<BackupManager>, pool: crate::database::Pool) -> Self {
        BackupScheduler {
            scheduled_tasks: Arc::new(RwLock::new(HashMap::new())),
            backup_manager,
            pool,
            shutdown_token: CancellationToken::new(),
        }
    }

    pub async fn schedule_rule(&self, rule_id: &str, cycle: &str, backup_time: serde_json::Value) {
        let next_run_time = match calculate_next_run_time(cycle, &backup_time) {
            Some(time) => time,
            None => return,
        };

        let task = ScheduledTask {
            config: ScheduleConfig {
                cycle: cycle.to_string(),
                backup_time,
            },
            next_run_time,
        };

        let mut tasks = self.scheduled_tasks.write().await;
        tasks.insert(rule_id.to_string(), task);
    }

    pub async fn remove_rule(&self, rule_id: &str) {
        let mut tasks = self.scheduled_tasks.write().await;
        tasks.remove(rule_id);
    }

    pub async fn get_next_run_time(&self, rule_id: &str) -> Option<String> {
        let tasks = self.scheduled_tasks.read().await;
        tasks.get(rule_id).map(|t| t.next_run_time.format("%Y-%m-%d %H:%M:%S").to_string())
    }

    pub async fn load_scheduled_rules(&self) {
        let conn = match self.pool.get() {
            Ok(c) => c,
            Err(_) => return,
        };

        let mut stmt = match conn.prepare(
            "SELECT br.id, br.cycle, br.backup_time FROM backup_rules br JOIN users u ON br.user_id = u.id WHERE u.expire_at IS NULL OR datetime(u.expire_at) > datetime('now')"
        ) {
            Ok(s) => s,
            Err(_) => return,
        };

        let rules: Vec<(String, String, String)> = stmt
            .query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?)))
            .map(|rows| rows.filter_map(|item| item.ok()).collect())
            .unwrap_or_default();

        for (rule_id, cycle, backup_time_str) in rules {
            let backup_time: serde_json::Value = match serde_json::from_str(&backup_time_str) {
                Ok(v) => v,
                Err(_) => continue,
            };
            self.schedule_rule(&rule_id, &cycle, backup_time).await;
        }
    }

    pub async fn run(&self) {
        loop {
            tokio::select! {
                _ = tokio::time::sleep(tokio::time::Duration::from_secs(60)) => {},
                _ = self.shutdown_token.cancelled() => {
                    break;
                },
            }

            let now = Utc::now();
            let tasks_to_run: Vec<String>;

            {
                let tasks = self.scheduled_tasks.read().await;
                tasks_to_run = tasks
                    .iter()
                    .filter(|(_, task)| task.next_run_time <= now)
                    .map(|(rule_id, _)| rule_id.clone())
                    .collect();
            }

            for rule_id in tasks_to_run {
                if self.backup_manager.is_task_running(&rule_id).await {
                    self.log_skipped_backup(&rule_id, "备份正在执行，忽略本次自动备份").ok();
                }

                {
                    let tasks = self.scheduled_tasks.read().await;
                    if let Some(task) = tasks.get(&rule_id) {
                        if let Some(next_time) = calculate_next_run_time(&task.config.cycle, &task.config.backup_time) {
                            drop(tasks);
                            let mut tasks_mut = self.scheduled_tasks.write().await;
                            if let Some(t) = tasks_mut.get_mut(&rule_id) {
                                t.next_run_time = next_time;
                            }
                        } else {
                            drop(tasks);
                            let mut tasks_mut = self.scheduled_tasks.write().await;
                            tasks_mut.remove(&rule_id);
                        }
                    }
                }

                if self.backup_manager.is_task_running(&rule_id).await {
                    continue;
                }

                if let Err(_) = self.backup_manager.start_task(&rule_id, "full").await {
                    continue;
                }
            }
        }
    }

    fn log_skipped_backup(&self, rule_id: &str, reason: &str) -> Result<(), String> {
        let conn = self.pool.get().map_err(|e| e.to_string())?;
        let now = Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
        let log_id = uuid::Uuid::new_v4().to_string();
        
        conn.execute(
            "INSERT INTO backup_logs (id, backup_rule_id, status, mode, started_at, finished_at, fail_reason) 
             VALUES (?1, ?2, 'failed', 'full', ?3, ?4, ?5)",
            params![log_id, rule_id, now, now, reason],
        ).map_err(|e| e.to_string())?;

        Ok(())
    }
}

fn calculate_next_run_time(cycle: &str, backup_time: &serde_json::Value) -> Option<chrono::DateTime<chrono::Utc>> {
    let now = Utc::now();
    let time_str = backup_time.get("time")?.as_str()?;
    let time_parts: Vec<u32> = time_str.split(':').filter_map(|s| s.parse().ok()).collect();
    if time_parts.len() < 2 {
        return None;
    }
    let hour = time_parts[0];
    let minute = time_parts[1];
    
    let target_time = NaiveTime::from_hms_opt(hour, minute, 0)?;
    
    match cycle {
        "daily" => {
            let next = now.date_naive().and_time(target_time);
            let result = if next <= now.naive_utc() {
                next + Duration::days(1)
            } else {
                next
            };
            Some(Utc.from_utc_datetime(&result))
        }
        "weekly" => {
            let week_day = backup_time.get("week_day")?.as_u64()? as u32;
            let current_weekday = now.weekday().num_days_from_monday() + 1;
            let days_ahead = if week_day >= current_weekday {
                week_day - current_weekday
            } else {
                7 - current_weekday + week_day
            };
            
            let next_date = now.date_naive() + Duration::days(days_ahead as i64);
            let next = next_date.and_time(target_time);
            let result = if next <= now.naive_utc() {
                next + Duration::days(7)
            } else {
                next
            };
            Some(Utc.from_utc_datetime(&result))
        }
        "monthly" => {
            let month_day = backup_time.get("month_day")?.as_u64()? as u32;
            let mut year = now.year();
            let mut month = now.month();
            
            let next_date = NaiveDate::from_ymd_opt(year, month, month_day)?;
            let next = next_date.and_time(target_time);
            
            let result = if next <= now.naive_utc() {
                month += 1;
                if month > 12 {
                    month = 1;
                    year += 1;
                }
                let next_date = NaiveDate::from_ymd_opt(year, month, month_day)?;
                next_date.and_time(target_time)
            } else {
                next
            };
            Some(Utc.from_utc_datetime(&result))
        }
        "yearly" => {
            let year_date = backup_time.get("year_date")?.as_str()?;
            let date_parts: Vec<u32> = year_date.split('-').filter_map(|s| s.parse().ok()).collect();
            if date_parts.len() < 2 {
                return None;
            }
            let target_month = date_parts[0];
            let target_day = date_parts[1];
            
            let mut year = now.year();
            let next_date = NaiveDate::from_ymd_opt(year, target_month, target_day)?;
            let next = next_date.and_time(target_time);
            
            let result = if next <= now.naive_utc() {
                year += 1;
                let next_date = NaiveDate::from_ymd_opt(year, target_month, target_day)?;
                next_date.and_time(target_time)
            } else {
                next
            };
            Some(Utc.from_utc_datetime(&result))
        }
        _ => None,
    }
}
