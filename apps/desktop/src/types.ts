export type LoaderType = "vanilla" | "fabric" | "forge" | "neoforge" | "quilt";

export interface InstanceSummary {
  id: string;
  name: string;
  directory_name: string;
  minecraft_version: string;
  loader_type: LoaderType;
  loader_version?: string;
  allocated_memory_mb: number;
  created_at: string;
  last_played_at?: string;
  /** Kaynak modpack ikonu (elle oluşturulan profillerde boş). */
  icon_url?: string;
  /** Kaynak modpack kapak görseli — profil kartında banner olarak kullanılır. */
  cover_url?: string;
  /** Kaynak projenin sağlayıcı kimliği (ör. Modrinth project_id). */
  project_id?: string;
}

export interface CreateInstanceRequest {
  name: string;
  minecraft_version: string;
  loader_type: LoaderType;
  loader_version?: string;
  allocated_memory_mb?: number;
  icon_url?: string;
  cover_url?: string;
  project_id?: string;
}

export interface SystemStatus {
  version: string;
  target_os: string;
  data_dir: string;
  instances_count: number;
  java_path?: string;
}

