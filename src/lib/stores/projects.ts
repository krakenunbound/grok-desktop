/**
 * Project list store - pinned / recent project folders.
 */
import { writable, derived, get } from "svelte/store";
import { invoke } from "@tauri-apps/api/core";
import { debugWarn } from "$lib/log";

export interface Project {
  id: string;
  name: string;
  path: string;
  pinned: boolean;
  archived: boolean;
  notes: string;
  project_type: string;
  last_modified: string | null;
  last_opened: string;
  last_chat_id: string | null;
}

export type ProjectSort = "last-opened" | "last-modified" | "name" | "type";

export const projects = writable<Project[]>([]);
export const activeProjectId = writable<string | null>(null);
export const projectSort = writable<ProjectSort>("last-opened");
export const showArchivedProjects = writable(false);

export const activeProject = derived(
  [projects, activeProjectId],
  ([$projects, $id]) => $projects.find((p) => p.id === $id) ?? null,
);

function normalizeProject(project: Project): Project {
  return {
    ...project,
    archived: !!project.archived,
    notes: project.notes ?? "",
    project_type: project.project_type || "Folder",
    last_modified: project.last_modified ?? null,
  };
}

function sortProjects(list: Project[], sort: ProjectSort): Project[] {
  return [...list].sort((a, b) => {
    if (sort === "name") return a.name.localeCompare(b.name);
    if (sort === "type") {
      const byType = (a.project_type || "Folder").localeCompare(b.project_type || "Folder");
      return byType || a.name.localeCompare(b.name);
    }
    if (sort === "last-modified") {
      return (b.last_modified ?? "").localeCompare(a.last_modified ?? "");
    }
    return b.last_opened.localeCompare(a.last_opened);
  });
}

function visible(project: Project, showArchived: boolean): boolean {
  return showArchived || !project.archived;
}

export const pinnedProjects = derived(
  [projects, projectSort, showArchivedProjects],
  ([$projects, $sort, $showArchived]) =>
    sortProjects(
      $projects.filter((project) => project.pinned && visible(project, $showArchived)),
      $sort,
    ),
);

export const recentProjects = derived(
  [projects, projectSort, showArchivedProjects],
  ([$projects, $sort, $showArchived]) =>
    sortProjects(
      $projects.filter((project) => !project.pinned && visible(project, $showArchived)),
      $sort,
    ),
);

function setStore(store: { projects: Project[] }) {
  projects.set((store.projects ?? []).map(normalizeProject));
}

export async function loadProjects(): Promise<void> {
  try {
    setStore(await invoke<{ projects: Project[] }>("list_projects"));
  } catch (e) {
    debugWarn("loadProjects", e);
  }
}

export async function addProject(path: string, name?: string): Promise<Project> {
  const project = normalizeProject(
    await invoke<Project>("add_project", { path, name: name ?? null }),
  );
  await loadProjects();
  activeProjectId.set(project.id);
  return project;
}

export async function removeProject(id: string): Promise<void> {
  setStore(await invoke<{ projects: Project[] }>("remove_project", { id }));
  if (get(activeProjectId) === id) activeProjectId.set(null);
}

export async function setPinned(id: string, pinned: boolean): Promise<void> {
  setStore(await invoke<{ projects: Project[] }>("set_project_pinned", { id, pinned }));
}

export async function setArchived(id: string, archived: boolean): Promise<void> {
  setStore(await invoke<{ projects: Project[] }>("set_project_archived", { id, archived }));
  if (archived && get(activeProjectId) === id) activeProjectId.set(null);
}

export async function updateProjectNotes(id: string, notes: string): Promise<void> {
  setStore(await invoke<{ projects: Project[] }>("update_project_notes", { id, notes }));
}

export async function touchProject(id: string, lastChatId?: string | null): Promise<void> {
  await invoke("touch_project", { id, lastChatId: lastChatId ?? null });
  await loadProjects();
}
