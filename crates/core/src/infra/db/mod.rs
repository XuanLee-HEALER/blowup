use sqlx::SqlitePool;
use std::path::Path;

/// Compiled migrator embedded at build time. Re-usable by test code in other
/// crates — call `blowup_core::infra::db::MIGRATOR.run(&pool).await` instead
/// of invoking `sqlx::migrate!("./migrations")` (which would look for a
/// `migrations` directory next to *that* crate's Cargo.toml and fail).
pub static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");

/// Initialize the SQLite database.
///
/// - If the DB file doesn't exist, create it and run all migrations.
/// - If the DB exists and migrations match, return the pool.
/// - If there's a version mismatch or corruption, return a descriptive error
///   (caller should show a dialog instead of panicking).
///
/// `data_dir` is where `blowup.db` lives. The caller resolves the platform
/// app-data directory (Tauri via `app.path().app_data_dir()`, server via its
/// own config path) and passes it here.
pub async fn init_db(data_dir: &Path) -> Result<SqlitePool, String> {
    std::fs::create_dir_all(data_dir).ok();

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

    match MIGRATOR.run(&pool).await {
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
