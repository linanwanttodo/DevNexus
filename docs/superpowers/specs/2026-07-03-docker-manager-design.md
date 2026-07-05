# Docker Container Management Module — Design Doc

## Overview

Add a Docker container management module to DevNexus, allowing users to manage Docker
containers, images, volumes, networks, and Compose projects through the existing GUI.

## Architecture

```
Frontend: src/routes/ContainerManager.svelte
  - 5-tab sidebar (Containers / Images / Volumes / Networks / Compose)
  - Each tab has its own list view, search, and action buttons
  - Modals for: logs viewer, terminal exec, pull image, build image, create volume/network

Backend: src-tauri/src/commands/container.rs
  - 20+ Tauri commands wrapping `docker` CLI
  - JSON-lines output parsing via serde_json
  - Follows existing std::process::Command pattern (like software.rs)

Icons: src/icons/ContainerIcons.svelte
  - Custom SVGs for container, image, volume, network, compose, terminal, logs
  - Status indicators (running green, paused yellow, stopped gray)
```

## Files Changed

| File | Change |
|------|--------|
| `src-tauri/src/commands/container.rs` | **NEW** — 20 Docker CLI commands |
| `src-tauri/src/commands/mod.rs` | Add `pub mod container;` |
| `src-tauri/src/lib.rs` | Register 19 new invoke handlers |
| `src/icons/ContainerIcons.svelte` | **NEW** — SVG icons for Docker |
| `src/routes/ContainerManager.svelte` | **NEW** — Full frontend page |
| `src/App.svelte` | Import + route for `/containers` |
| `src/components/Sidebar.svelte` | Nav item with Docker logo icon |
| `src/locales/zh.json` | Docker i18n section + nav key |
| `src/locales/en.json` | Docker i18n section + nav key |
| `src/locales/ru.json` | Docker i18n section + nav key |

## Backend Commands

| Command | Description | Docker CLI Equivalent |
|---------|-------------|----------------------|
| `check_docker` | Check if Docker is installed + running | `docker --version` + `docker info` |
| `list_containers` | List all/running containers | `docker ps [-a] --format '{{json .}}'` |
| `container_action` | start/stop/restart/pause/unpause/rm | `docker <action> <name>` |
| `get_container_logs` | Get container logs | `docker logs --tail N <name>` |
| `exec_in_container` | Run command in container | `docker exec <name> sh -c <cmd>` |
| `list_images` | List images | `docker images --format '{{json .}}'` |
| `pull_image` | Pull image | `docker pull <image>` |
| `remove_image` | Remove image | `docker rmi [-f] <id>` |
| `build_image` | Build from Dockerfile | `docker build -t <tag> <path>` |
| `tag_image` | Tag image | `docker tag <id> <tag>` |
| `push_image` | Push image | `docker push <tag>` |
| `list_volumes` | List volumes | `docker volume ls --format '{{json .}}'` |
| `volume_action` | Create/remove volume | `docker volume create/rm <name>` |
| `list_networks` | List networks | `docker network ls --format '{{json .}}'` |
| `network_action` | Create/remove network | `docker network create/rm <name>` |
| `compose_up` | Compose up | `docker compose [-f <file>] up -d` |
| `compose_down` | Compose down | `docker compose [-f <file>] down` |
| `compose_ps` | Compose ps | `docker compose [-f <file>] ps --format '{{json .}}'` |
| `compose_logs` | Compose logs | `docker compose [-f <file>] logs --tail N` |

## UI Layout

```
┌──────────────┬──────────────────────────────────────────────┐
│  Sidebar      │  Main Content                               │
│  Tabs:        │                                              │
│  ┌──────────┐ │  ┌─── Title + Action Bar ──────────────┐    │
│  │ ▫ Containers│  │  "Docker Containers"     [Search] [↻] │    │
│  │ ▫ Images    │  └───────────────────────────────────────┘    │
│  │ ▫ Volumes   │  ┌─── Data Table ─────────────────────────┐  │
│  │ ▫ Networks  │  │ Name │ Status │ Image │ Ports │ Actions│  │
│  │ ▫ Compose   │  │ nginx │ 🟢    │ nginx  │ 80→8080│ [..]  │  │
│  └──────────┘ │  └─────────────────────────────────────────┘  │
└──────────────┴──────────────────────────────────────────────┘
```

## Key Design Decisions

1. **Docker CLI over SDK** — consistent with existing `std::process::Command` pattern
2. **Modal dialogs** — logs, terminal, pull, build use overlay dialogs (not new routes)
3. **Auto-refresh** — manual refresh per tab; no auto-poll (conservative)
4. **i18n** — full zh/en/ru translations for all visible strings
5. **Error states** — Docker not installed, not running, connection errors all handled with dedicated screens
6. **SVG icons** — custom ContainerIcons component for Docker-specific actions
