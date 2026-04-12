use super::{GraphData, GraphLink, GraphNode};
use sqlx::SqlitePool;
use std::collections::{HashMap, HashSet, VecDeque};

#[derive(sqlx::FromRow)]
struct RelRow {
    from_id: i64,
    to_id: i64,
    relation_type: String,
}

#[derive(sqlx::FromRow)]
struct EntryName {
    id: i64,
    name: String,
}

/// Build graph data from entries + relations.
/// If `center_id` is given, BFS-expand 2 layers from that node.
/// Otherwise return the full graph.
#[tauri::command]
pub async fn get_graph_data(
    center_id: Option<i64>,
    pool: tauri::State<'_, SqlitePool>,
) -> Result<GraphData, String> {
    let all_rels =
        sqlx::query_as::<_, RelRow>("SELECT from_id, to_id, relation_type FROM relations")
            .fetch_all(pool.inner())
            .await
            .map_err(|e| e.to_string())?;

    // Build adjacency for BFS
    let mut adj: HashMap<i64, Vec<(i64, &RelRow)>> = HashMap::new();
    for r in &all_rels {
        adj.entry(r.from_id).or_default().push((r.to_id, r));
        adj.entry(r.to_id).or_default().push((r.from_id, r));
    }

    // Determine which nodes to include
    let node_ids: HashSet<i64> = if let Some(cid) = center_id {
        let mut visited = HashSet::new();
        let mut queue = VecDeque::new();
        visited.insert(cid);
        queue.push_back((cid, 0u32));
        while let Some((nid, depth)) = queue.pop_front() {
            if depth >= 2 {
                continue;
            }
            if let Some(neighbors) = adj.get(&nid) {
                for &(neighbor, _) in neighbors {
                    if visited.insert(neighbor) {
                        queue.push_back((neighbor, depth + 1));
                    }
                }
            }
        }
        visited
    } else {
        // All nodes that participate in at least one relation
        let mut ids = HashSet::new();
        for r in &all_rels {
            ids.insert(r.from_id);
            ids.insert(r.to_id);
        }
        ids
    };

    if node_ids.is_empty() {
        return Ok(GraphData {
            nodes: vec![],
            links: vec![],
        });
    }

    // Fetch names for included nodes
    let names = sqlx::query_as::<_, EntryName>("SELECT id, name FROM entries")
        .fetch_all(pool.inner())
        .await
        .map_err(|e| e.to_string())?;
    let name_map: HashMap<i64, String> = names.into_iter().map(|e| (e.id, e.name)).collect();

    // Count relations per node for weight
    let mut rel_count: HashMap<i64, usize> = HashMap::new();
    for r in &all_rels {
        if node_ids.contains(&r.from_id) && node_ids.contains(&r.to_id) {
            *rel_count.entry(r.from_id).or_default() += 1;
            *rel_count.entry(r.to_id).or_default() += 1;
        }
    }
    let max_count = rel_count.values().copied().max().unwrap_or(1) as f64;

    let nodes: Vec<GraphNode> = node_ids
        .iter()
        .map(|&id| {
            let label = name_map.get(&id).cloned().unwrap_or_default();
            let count = *rel_count.get(&id).unwrap_or(&0) as f64;
            GraphNode {
                id: format!("e{id}"),
                label,
                weight: 0.5 + (count / max_count) * 2.5,
            }
        })
        .collect();

    let links: Vec<GraphLink> = all_rels
        .iter()
        .filter(|r| node_ids.contains(&r.from_id) && node_ids.contains(&r.to_id))
        .map(|r| GraphLink {
            source: format!("e{}", r.from_id),
            target: format!("e{}", r.to_id),
            relation_type: r.relation_type.clone(),
        })
        .collect();

    Ok(GraphData { nodes, links })
}

#[cfg(test)]
mod tests {
    use sqlx::SqlitePool;

    async fn setup() -> SqlitePool {
        let pool = SqlitePool::connect(":memory:").await.unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        pool
    }

    #[tokio::test]
    async fn graph_empty() {
        let pool = setup().await;
        let rels: Vec<super::RelRow> =
            sqlx::query_as("SELECT from_id, to_id, relation_type FROM relations")
                .fetch_all(&pool)
                .await
                .unwrap();
        assert!(rels.is_empty());
    }

    #[tokio::test]
    async fn graph_with_relations() {
        let pool = setup().await;
        let a = sqlx::query("INSERT INTO entries (name) VALUES ('A')")
            .execute(&pool)
            .await
            .unwrap()
            .last_insert_rowid();
        let b = sqlx::query("INSERT INTO entries (name) VALUES ('B')")
            .execute(&pool)
            .await
            .unwrap()
            .last_insert_rowid();
        let c = sqlx::query("INSERT INTO entries (name) VALUES ('C')")
            .execute(&pool)
            .await
            .unwrap()
            .last_insert_rowid();

        sqlx::query("INSERT INTO relations (from_id, to_id, relation_type) VALUES (?, ?, 'x')")
            .bind(a)
            .bind(b)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO relations (from_id, to_id, relation_type) VALUES (?, ?, 'y')")
            .bind(b)
            .bind(c)
            .execute(&pool)
            .await
            .unwrap();

        let rels: Vec<super::RelRow> =
            sqlx::query_as("SELECT from_id, to_id, relation_type FROM relations")
                .fetch_all(&pool)
                .await
                .unwrap();
        assert_eq!(rels.len(), 2);

        // All nodes referenced
        let mut ids = std::collections::HashSet::new();
        for r in &rels {
            ids.insert(r.from_id);
            ids.insert(r.to_id);
        }
        assert_eq!(ids.len(), 3);
    }

    #[test]
    fn weight_normalization() {
        let counts = [2usize, 4, 1];
        let max = *counts.iter().max().unwrap() as f64;
        let weights: Vec<f64> = counts
            .iter()
            .map(|&c| 0.5 + (c as f64 / max) * 2.5)
            .collect();
        assert!((weights[1] - 3.0).abs() < f64::EPSILON);
    }
}
