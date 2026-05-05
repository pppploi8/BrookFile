use crate::backup::{BackupManager, BackupScheduler};
use crate::database::Pool;
use crate::models::{
    BackupRuleModel, NotebookModel, RecycleBinModel, ShareModel,
    SystemConfigModel, UploadCacheModel, UserModel, VaultModel, WebDavConfigModel,
};
use crate::restore::RestoreManager;
use crate::search::SearchManager;
use crate::session_manager::SessionManager;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;

pub struct ShareTokenEntry {
    pub share_code: String,
    pub expires_at: std::time::Instant,
}

pub struct AppState {
    pub user_model: UserModel,
    pub system_config_model: SystemConfigModel,
    pub upload_cache_model: UploadCacheModel,
    pub backup_rule_model: BackupRuleModel,
    pub recycle_bin_model: RecycleBinModel,
    pub vault_model: VaultModel,
    pub notebook_model: NotebookModel,
    pub share_model: ShareModel,
    pub webdav_config_model: WebDavConfigModel,
    pub notebook_key_cache: Arc<Mutex<HashMap<String, (Vec<u8>, std::time::Instant)>>>,
    pub share_tokens: Arc<Mutex<HashMap<String, ShareTokenEntry>>>,
    pub attachment_secret: String,
    pub session_manager: Arc<SessionManager>,
    pub backup_manager: Arc<BackupManager>,
    pub backup_scheduler: Arc<BackupScheduler>,
    pub restore_manager: Arc<RestoreManager>,
    pub search_manager: Arc<SearchManager>,
}

impl AppState {
    pub fn new(
        pool: Pool,
        session_manager: Arc<SessionManager>,
        backup_manager: Arc<BackupManager>,
        backup_scheduler: Arc<BackupScheduler>,
        restore_manager: Arc<RestoreManager>,
        fulltext_search_enabled: bool,
    ) -> Self {
        let search_manager = Arc::new(SearchManager::new(fulltext_search_enabled));
        let attachment_secret = uuid::Uuid::new_v4().to_string();
        AppState {
            user_model: UserModel::new(&pool),
            system_config_model: SystemConfigModel::new(&pool),
            upload_cache_model: UploadCacheModel::new(&pool),
            backup_rule_model: BackupRuleModel::new(&pool),
            recycle_bin_model: RecycleBinModel::new(&pool),
            vault_model: VaultModel::new(&pool),
            notebook_model: NotebookModel::new(&pool),
            share_model: ShareModel::new(&pool),
            webdav_config_model: WebDavConfigModel::new(&pool),
            notebook_key_cache: Arc::new(Mutex::new(HashMap::new())),
            share_tokens: Arc::new(Mutex::new(HashMap::new())),
            session_manager,
            backup_manager,
            backup_scheduler,
            restore_manager,
            search_manager,
            attachment_secret,
        }
    }

    pub fn is_initialized(&self) -> Result<bool, String> {
        self.system_config_model.is_initialized()
    }

    pub fn set_initialized(&self, value: bool) -> Result<(), String> {
        self.system_config_model.set_initialized(value)
    }
}
