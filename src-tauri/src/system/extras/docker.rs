use serde::Serialize;

use crate::maintenance::commands::execute_system_command;

// Un demon Docker qui ne repond pas (Docker Desktop en train de demarrer, ou
// plante) faisait pendre ces appels SANS LIMITE : `Command::output()` attend la
// fin du processus, point. Tout passe donc par `execute_system_command`, qui
// tue le processus au bout du delai. Meme famille de bugs que `monitor.rs`.
//
// 20 s pour une lecture, 60 s pour une action : `docker stop` attend par defaut
// dix secondes que le conteneur s'arrete tout seul avant de le tuer.
const LECTURE: u64 = 20;
const ACTION: u64 = 60;

// ─── Docker Manager ───────────────────────────────────────────────────────────

#[derive(Serialize)]
pub struct DockerContainer {
    pub id: String,
    pub name: String,
    pub image: String,
    pub status: String,
    pub ports: String,
    pub created: String,
}

#[derive(Serialize)]
pub struct DockerImage {
    pub id: String,
    pub repository: String,
    pub tag: String,
    pub size: String,
    pub created: String,
}

#[derive(Serialize)]
pub struct DockerInfo {
    pub available: bool,
    pub version: String,
    pub containers: Vec<DockerContainer>,
    pub images: Vec<DockerImage>,
}

// Anti-freeze : docker CLI est bloquant — jamais inline sur le thread de commande.
#[tauri::command]
pub async fn get_docker_info() -> Result<DockerInfo, String> {
    tokio::task::spawn_blocking(get_docker_info_blocking)
        .await
        .map_err(|e| e.to_string())?
}

fn get_docker_info_blocking() -> Result<DockerInfo, String> {
    let version_out =
        execute_system_command("docker", &["version", "--format", "{{.Server.Version}}"], LECTURE);

    let (available, version) = match version_out {
        Ok(o) if o.success => (true, o.stdout.trim().to_string()),
        _ => return Ok(DockerInfo { available: false, version: String::new(), containers: vec![], images: vec![] }),
    };

    let containers = parse_docker_ps();
    let images = parse_docker_images();

    Ok(DockerInfo { available, version, containers, images })
}

fn parse_docker_ps() -> Vec<DockerContainer> {
    let out = execute_system_command(
        "docker",
        &["ps", "-a", "--format", "{{.ID}}\t{{.Names}}\t{{.Image}}\t{{.Status}}\t{{.Ports}}\t{{.CreatedAt}}"],
        LECTURE,
    )
    .ok();
    let text = out.map(|o| o.stdout).unwrap_or_default();
    text.lines().filter(|l| !l.is_empty()).map(|line| {
        let parts: Vec<&str> = line.splitn(6, '\t').collect();
        DockerContainer {
            id: parts.first().unwrap_or(&"").to_string(),
            name: parts.get(1).unwrap_or(&"").to_string(),
            image: parts.get(2).unwrap_or(&"").to_string(),
            status: parts.get(3).unwrap_or(&"").to_string(),
            ports: parts.get(4).unwrap_or(&"").to_string(),
            created: parts.get(5).unwrap_or(&"").to_string(),
        }
    }).collect()
}

fn parse_docker_images() -> Vec<DockerImage> {
    let out = execute_system_command(
        "docker",
        &["images", "--format", "{{.ID}}\t{{.Repository}}\t{{.Tag}}\t{{.Size}}\t{{.CreatedAt}}"],
        LECTURE,
    )
    .ok();
    let text = out.map(|o| o.stdout).unwrap_or_default();
    text.lines().filter(|l| !l.is_empty()).map(|line| {
        let parts: Vec<&str> = line.splitn(5, '\t').collect();
        DockerImage {
            id: parts.first().unwrap_or(&"").to_string(),
            repository: parts.get(1).unwrap_or(&"").to_string(),
            tag: parts.get(2).unwrap_or(&"").to_string(),
            size: parts.get(3).unwrap_or(&"").to_string(),
            created: parts.get(4).unwrap_or(&"").to_string(),
        }
    }).collect()
}

// Anti-freeze : docker CLI est bloquant — jamais inline sur le thread de commande.
#[tauri::command]
pub async fn docker_container_action(container_id: String, action: String) -> Result<String, String> {
    tokio::task::spawn_blocking(move || docker_container_action_blocking(container_id, action))
        .await
        .map_err(|e| e.to_string())?
}

fn docker_container_action_blocking(container_id: String, action: String) -> Result<String, String> {
    let valid_actions = ["start", "stop", "restart", "rm", "kill"];
    if !valid_actions.contains(&action.as_str()) {
        return Err(format!("Action invalide: {}", action));
    }
    let out = execute_system_command("docker", &[action.as_str(), &container_id], ACTION)
        .map_err(|e| e.to_string())?;
    Ok(out.stdout.trim().to_string())
}

// Anti-freeze : docker CLI est bloquant — jamais inline sur le thread de commande.
#[tauri::command]
pub async fn docker_image_remove(image_id: String) -> Result<String, String> {
    tokio::task::spawn_blocking(move || docker_image_remove_blocking(image_id))
        .await
        .map_err(|e| e.to_string())?
}

fn docker_image_remove_blocking(image_id: String) -> Result<String, String> {
    let out = execute_system_command("docker", &["rmi", &image_id], ACTION)
        .map_err(|e| e.to_string())?;
    Ok(out.stdout.trim().to_string())
}

// Anti-freeze : docker CLI est bloquant — jamais inline sur le thread de commande.
#[tauri::command]
pub async fn docker_container_logs(container_id: String, lines: u32) -> Result<String, String> {
    tokio::task::spawn_blocking(move || docker_container_logs_blocking(container_id, lines))
        .await
        .map_err(|e| e.to_string())?
}

fn docker_container_logs_blocking(container_id: String, lines: u32) -> Result<String, String> {
    let n = lines.min(500).to_string();
    let out = execute_system_command(
        "docker",
        &["logs", "--tail", &n, "--timestamps", &container_id],
        LECTURE,
    )
    .map_err(|e| e.to_string())?;
    Ok(if out.stdout.is_empty() { out.stderr } else { out.stdout })
}
