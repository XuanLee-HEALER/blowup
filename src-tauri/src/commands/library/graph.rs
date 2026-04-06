use super::{GraphData, GraphLink, GraphNode};
use sqlx::SqlitePool;
use std::collections::HashMap;

#[derive(sqlx::FromRow)]
struct PersonFilmRow {
    person_id: i64,
    person_name: String,
    primary_role: String,
    film_id: i64,
    film_title: String,
    role: String,
}

#[tauri::command]
pub async fn get_graph_data(pool: tauri::State<'_, SqlitePool>) -> Result<GraphData, String> {
    let pf_rows = sqlx::query_as::<_, PersonFilmRow>(
        "SELECT pf.person_id, p.name as person_name, p.primary_role,
                pf.film_id, f.title as film_title, pf.role
         FROM person_films pf
         JOIN people p ON p.id = pf.person_id
         JOIN films f ON f.id = pf.film_id",
    )
    .fetch_all(pool.inner())
    .await
    .map_err(|e| e.to_string())?;

    let mut person_film_counts: HashMap<i64, i64> = HashMap::new();
    let mut person_meta: HashMap<i64, (String, String)> = HashMap::new();
    let mut film_meta: HashMap<i64, String> = HashMap::new();

    for row in &pf_rows {
        *person_film_counts.entry(row.person_id).or_insert(0) += 1;
        person_meta
            .entry(row.person_id)
            .or_insert_with(|| (row.person_name.clone(), row.primary_role.clone()));
        film_meta
            .entry(row.film_id)
            .or_insert_with(|| row.film_title.clone());
    }

    let max_count = person_film_counts.values().copied().max().unwrap_or(1) as f64;

    let mut nodes: Vec<GraphNode> = film_meta
        .iter()
        .map(|(id, title)| GraphNode {
            id: format!("f{id}"),
            label: title.clone(),
            node_type: "film".to_string(),
            role: None,
            weight: 1.0,
        })
        .collect();

    for (person_id, film_count) in &person_film_counts {
        let (name, primary_role) = person_meta.get(person_id).unwrap();
        let weight = 0.5 + (*film_count as f64 / max_count) * 2.5;
        nodes.push(GraphNode {
            id: format!("p{person_id}"),
            label: name.clone(),
            node_type: "person".to_string(),
            role: Some(primary_role.clone()),
            weight,
        });
    }

    let links: Vec<GraphLink> = pf_rows
        .iter()
        .map(|row| GraphLink {
            source: format!("p{}", row.person_id),
            target: format!("f{}", row.film_id),
            role: row.role.clone(),
        })
        .collect();

    Ok(GraphData { nodes, links })
}

#[cfg(test)]
mod tests {
    #[test]
    fn weight_normalization() {
        let counts = vec![2i64, 4, 1];
        let max = *counts.iter().max().unwrap() as f64;
        let weights: Vec<f64> = counts
            .iter()
            .map(|&c| 0.5 + (c as f64 / max) * 2.5)
            .collect();
        assert!((weights[1] - 3.0).abs() < f64::EPSILON);
        assert!((weights[2] - (0.5 + 0.25 * 2.5)).abs() < f64::EPSILON);
    }
}
