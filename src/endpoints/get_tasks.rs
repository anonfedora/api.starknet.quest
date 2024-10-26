use crate::{models::AppState, utils::get_error};
use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::{IntoResponse, Json},
};
use axum_auto_routes::route;
use futures::stream::StreamExt;
use mongodb::bson::{doc, from_document, Document};
use serde::{Deserialize, Serialize};
use starknet::core::types::FieldElement;
use std::sync::Arc;

#[derive(Debug, Serialize, Deserialize)]
pub struct UserTask {
    id: u32,
    quest_id: u32,
    name: String,
    href: String,
    cta: String,
    verify_endpoint: String,
    verify_endpoint_type: String,
    verify_redirect: Option<String>,
    desc: String,
    completed: bool,
    quiz_name: Option<i64>,
    calls: Option<Vec<String>>,
    contracts: Option<Vec<String>>,
    api_url: Option<String>,
    regex: Option<String>,
}

#[derive(Deserialize)]
pub struct GetTasksQuery {
    quest_id: u32,
    addr: FieldElement,
}

#[route(get, "/get_tasks")]
pub async fn handler(
    State(state): State<Arc<AppState>>,
    Query(query): Query<GetTasksQuery>,
) -> impl IntoResponse {
    let pipeline = vec![
        doc! { "$match": { "quest_id": query.quest_id } },
        doc! {
            "$lookup": {
                "from": "completed_tasks",
                "let": { "task_id": "$id" },
                "pipeline": [
                    {
                        "$match": {
                            "$expr": { "$eq": [ "$task_id", "$$task_id" ] },
                            "address": query.addr.to_string(),
                        },
                    },
                ],
                "as": "completed",
            }
        },
        doc! {
            "$lookup": {
                "from": "quests",
                "localField": "quest_id",
                "foreignField": "id",
                "as": "quest"
            }
        },
        doc! { "$unwind": "$quest" },
        doc! { "$match": { "quest.disabled": false } },
        doc! {
            "$addFields": {
                "sort_order": doc! {
                    "$switch": {
                        "branches": [
                            {
                                "case": doc! { "$eq": ["$verify_endpoint_type", "quiz"] },
                                "then": 1
                            },
                            {
                                "case": doc! { "$eq": ["$verify_endpoint_type", "default"] },
                                "then": 2
                            }
                        ],
                        "default": 3
                    }
                }
            }
        },
        doc! { "$sort": { "sort_order": 1 } },
        doc! { "$sort": { "overwrite_order": 1 } },
        doc! {
            "$project": {
                "_id": 0,
                "id": 1,
                "quest_id": 1,
                "name": 1,
                "href": 1,
                "cta": 1,
                "verify_endpoint": 1,
                "verify_redirect" : 1,
                "verify_endpoint_type": 1,
                "desc": 1,
                "completed": { "$gt": [ { "$size": "$completed" }, 0 ] },
                "quiz_name": 1,
                // Added missing fields to projection
                "calls": 1,
                "contracts": 1,
                "api_url": 1,
                "regex": 1,
            }
        },
    ];
    let tasks_collection = state.db.collection::<Document>("tasks");
    match tasks_collection.aggregate(pipeline, None).await {
        Ok(mut cursor) => {
            let mut tasks: Vec<UserTask> = Vec::new();
            while let Some(result) = cursor.next().await {
                match result {
                    Ok(document) => {
                        if let Ok(task) = from_document::<UserTask>(document) {
                            tasks.push(task);
                        }
                    }
                    _ => continue,
                }
            }
            if tasks.is_empty() {
                get_error("No tasks found for this quest_id".to_string())
            } else {
                (StatusCode::OK, Json(tasks)).into_response()
            }
        }
        Err(_) => get_error("Error querying tasks".to_string()),
    }
}
