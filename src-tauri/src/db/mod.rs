// src-tauri/src/db/mod.rs
use sqlx::SqlitePool;
use tauri::AppHandle;
use tauri::Manager;

pub async fn init_db(app: &AppHandle) -> Result<SqlitePool, sqlx::Error> {
    let data_dir = app
        .path()
        .app_data_dir()
        .expect("could not resolve app data dir");
    std::fs::create_dir_all(&data_dir).ok();

    let db_path = data_dir.join("blowup.db");
    let url = format!(
        "sqlite://{}?mode=rwc",
        db_path.to_str().expect("non-utf8 db path")
    );

    let pool = SqlitePool::connect(&url).await?;
    sqlx::migrate!("./migrations").run(&pool).await?;
    Ok(pool)
}
