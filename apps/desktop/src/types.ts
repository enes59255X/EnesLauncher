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
}

export interface CreateInstanceRequest {
  name: string;
  minecraft_version: string;
  loader_type: LoaderType;
  loader_version?: string;
  allocated_memory_mb?: number;
}

export interface SystemStatus {
  version: string;
  target_os: string;
  data_dir: string;
  instances_count: number;
  java_path?: string;
}

