use sqlx::SqlitePool;
use tauri::AppHandle;
use tauri::Manager;

/// Initialize the SQLite database.
///
/// - If the DB file doesn't exist, create it and run all migrations.
/// - If the DB exists and migrations match, return the pool.
/// - If there's a version mismatch or corruption, return a descriptive error
///   (caller should show a dialog instead of panicking).
pub async fn init_db(app: &AppHandle) -> Result<SqlitePool, String> {
    let data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("无法解析应用数据目录: {}", e))?;
    std::fs::create_dir_all(&data_dir).ok();

    let db_path = data_dir.join("blowup.db");
    let db_exists = db_path.exists();
    let url = format!(
        "sqlite://{}?mode=rwc",
        db_path
            .to_str()
            .ok_or_else(|| "数据库路径包含非法字符".to_string())?
    );

    let pool = SqlitePool::connect(&url)
        .await
        .map_err(|e| format!("无法连接数据库: {}", e))?;

    match sqlx::migrate!("./migrations").run(&pool).await {
        Ok(_) => Ok(pool),
        Err(sqlx::migrate::MigrateError::VersionMismatch(ver)) => {
            if db_exists {
                Err(format!(
                    "数据库版本不兼容（迁移版本 {}）。\n\n\
                     可能原因：数据库由不同版本的应用创建。\n\
                     请备份并删除数据库文件后重试：\n{}",
                    ver,
                    db_path.display()
                ))
            } else {
                // Shouldn't happen for fresh DB, but handle anyway
                Err(format!("数据库迁移失败（版本 {}）", ver))
            }
        }
        Err(e) => Err(format!(
            "数据库迁移失败: {}\n\n数据库路径: {}",
            e,
            db_path.display()
        )),
    }
}
