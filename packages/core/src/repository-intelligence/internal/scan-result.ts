/** Shared shape every scanner returns, so the orchestrator can assemble a fingerprint. */

export interface DirListing {
  dir: string;
  entries: string[];
}

export interface ScanResult<TProfile> {
  profile: TProfile;
  filesRead: string[];
  dirsListed: DirListing[];
}
