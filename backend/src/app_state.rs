use crate::backup::{BackupManager, BackupScheduler};
use crate::ai::AiToolRegistry;
use crate::database::Pool;
use crate::handlers::note_ws::NoteRoomManager;
use crate::models::{
    AiModelModel, AiProviderModel, BackupRuleModel, NotebookModel, RecycleBinModel, ShareModel,
    SystemConfigModel, UploadCacheModel, UserModel, VaultModel, WebDavConfigModel, WebDavCorsModel,
};
use crate::restore::RestoreManager;
use crate::search::SearchManager;
use crate::session_manager::SessionManager;
use crate::ebook::EbookScanManager;
use std::collections::HashMap;
use std::collections::HashSet;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::RwLock;

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
    pub webdav_cors_model: WebDavCorsModel,
    pub ai_provider_model: AiProviderModel,
    pub ai_model_model: AiModelModel,
    pub ai_tools: Arc<AiToolRegistry>,
    pub ai_chat_lock: Arc<tokio::sync::Mutex<()>>,
    pub ai_page_query_pending:
        Arc<tokio::sync::Mutex<HashMap<String, tokio::sync::oneshot::Sender<String>>>>,
    pub note_rooms: Arc<NoteRoomManager>,
    pub cors_origins: Arc<RwLock<HashSet<String>>>,
    pub notebook_key_cache: Arc<Mutex<HashMap<String, (Vec<u8>, std::time::Instant)>>>,
    pub share_tokens: Arc<Mutex<HashMap<String, ShareTokenEntry>>>,
    pub attachment_secret: String,
    pub session_manager: Arc<SessionManager>,
    pub backup_manager: Arc<BackupManager>,
    pub backup_scheduler: Arc<BackupScheduler>,
    pub restore_manager: Arc<RestoreManager>,
    pub search_manager: Arc<SearchManager>,
    pub ebook_scan_manager: Arc<EbookScanManager>,
}

impl AppState {
    pub fn new(
        pool: Pool,
        session_manager: Arc<SessionManager>,
        backup_manager: Arc<BackupManager>,
        backup_scheduler: Arc<BackupScheduler>,
        restore_manager: Arc<RestoreManager>,
        fulltext_search_enabled: bool,
        ebook_scan_manager: Arc<EbookScanManager>,
    ) -> Self {
        let search_manager = Arc::new(SearchManager::new(fulltext_search_enabled));
        let attachment_secret = uuid::Uuid::new_v4().to_string();
        let webdav_cors_model = WebDavCorsModel::new(&pool);
        let cors_origins = Arc::new(RwLock::new(webdav_cors_model.all_origins().unwrap_or_default()));
        let note_rooms = Arc::new(NoteRoomManager::new());
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
            webdav_cors_model,
            ai_provider_model: AiProviderModel::new(&pool),
            ai_model_model: AiModelModel::new(&pool),
            ai_tools: {
                let registry = AiToolRegistry::new();
                registry.register(Arc::new(
                    crate::ebook::ai_tools::EbookToolProvider::new(&pool),
                ));
                Arc::new(registry)
            },
            ai_chat_lock: Arc::new(tokio::sync::Mutex::new(())),
            ai_page_query_pending: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
            note_rooms,
            cors_origins,
            notebook_key_cache: Arc::new(Mutex::new(HashMap::new())),
            share_tokens: Arc::new(Mutex::new(HashMap::new())),
            session_manager,
            backup_manager,
            backup_scheduler,
            restore_manager,
            search_manager,
            ebook_scan_manager,
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
