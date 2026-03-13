use crate::error::{AppError, AppResult};
use crate::middleware::AuthenticatedUser;
use crate::models::{AddGroupMemberRequest, CreateGroupRequest};
use crate::routes::vaults::AppState;
use actix_web::{delete, get, post, web, HttpMessage, HttpRequest, HttpResponse};

fn require_authenticated_user(req: &HttpRequest) -> AppResult<AuthenticatedUser> {
    req.extensions()
        .get::<AuthenticatedUser>()
        .cloned()
        .ok_or_else(|| AppError::Unauthorized("Authentication required".to_string()))
}

#[get("/api/groups")]
async fn list_groups(state: web::Data<AppState>, req: HttpRequest) -> AppResult<HttpResponse> {
    let user = require_authenticated_user(&req)?;
    let groups = state.db.list_groups_for_user(&user.user_id).await?;
    Ok(HttpResponse::Ok().json(groups))
}

#[post("/api/groups")]
async fn create_group(
    state: web::Data<AppState>,
    req: HttpRequest,
    body: web::Json<CreateGroupRequest>,
) -> AppResult<HttpResponse> {
    let user = require_authenticated_user(&req)?;
    let group = state.db.create_group(&body.name, &user.user_id).await?;
    Ok(HttpResponse::Created().json(group))
}

#[get("/api/groups/{group_id}/members")]
async fn list_group_members(
    state: web::Data<AppState>,
    req: HttpRequest,
    group_id: web::Path<String>,
) -> AppResult<HttpResponse> {
    let user = require_authenticated_user(&req)?;
    let group_id = group_id.into_inner();
    let visible_groups = state.db.list_groups_for_user(&user.user_id).await?;
    if !visible_groups.iter().any(|group| group.id == group_id) {
        return Err(AppError::Forbidden(
            "You do not have access to this group".to_string(),
        ));
    }

    let members = state.db.list_group_members(&group_id).await?;
    Ok(HttpResponse::Ok().json(members))
}

#[post("/api/groups/{group_id}/members")]
async fn add_group_member(
    state: web::Data<AppState>,
    req: HttpRequest,
    group_id: web::Path<String>,
    body: web::Json<AddGroupMemberRequest>,
) -> AppResult<HttpResponse> {
    let user = require_authenticated_user(&req)?;
    let group_id = group_id.into_inner();
    if !state.db.is_group_manager(&group_id, &user.user_id).await? {
        return Err(AppError::Forbidden(
            "Only the group owner can manage group membership".to_string(),
        ));
    }

    let target_user_id = if let Some(user_id) = &body.user_id {
        user_id.clone()
    } else if let Some(username) = &body.username {
        state
            .db
            .get_user_by_username(username)
            .await?
            .map(|(id, _)| id)
            .ok_or_else(|| AppError::NotFound(format!("User {} not found", username)))?
    } else {
        return Err(AppError::InvalidInput(
            "Provide either user_id or username".to_string(),
        ));
    };

    state.db.add_user_to_group(&group_id, &target_user_id).await?;
    let members = state.db.list_group_members(&group_id).await?;
    Ok(HttpResponse::Ok().json(members))
}

#[delete("/api/groups/{group_id}/members/{user_id}")]
async fn remove_group_member(
    state: web::Data<AppState>,
    req: HttpRequest,
    path: web::Path<(String, String)>,
) -> AppResult<HttpResponse> {
    let user = require_authenticated_user(&req)?;
    let (group_id, user_id) = path.into_inner();
    if !state.db.is_group_manager(&group_id, &user.user_id).await? {
        return Err(AppError::Forbidden(
            "Only the group owner can manage group membership".to_string(),
        ));
    }

    state.db.remove_user_from_group(&group_id, &user_id).await?;
    Ok(HttpResponse::NoContent().finish())
}

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(list_groups)
        .service(create_group)
        .service(list_group_members)
        .service(add_group_member)
        .service(remove_group_member);
}
