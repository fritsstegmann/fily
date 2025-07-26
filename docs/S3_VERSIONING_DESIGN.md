# S3-Compatible Versioning Implementation Plan

This document outlines the requirements and implementation plan for adding S3-compatible versioning support to Fily.

## Current Architecture Analysis

The current implementation uses a simple flat file system approach:
- Files stored as `{location}/{bucket}/{file}` 
- Direct filesystem operations (read/write/delete)
- No metadata tracking beyond what the filesystem provides

## What S3 Versioning Requires

### 1. Storage Architecture Changes (Major)

**Current Problem**: Files overwrite each other directly on disk

**Required Changes**:
- Implement a version-aware storage backend
- Generate unique version IDs (UUIDs or timestamps)
- Store multiple versions of the same logical object

**Proposed Structure**:
```
data/
├── buckets/
│   └── my-bucket/
│       ├── .bucket-config.json  # Versioning enabled/disabled
│       └── objects/
│           └── my-file.txt/
│               ├── versions/
│               │   ├── v1-abc123-20240101T120000Z.dat
│               │   ├── v2-def456-20240101T130000Z.dat
│               │   └── v3-ghi789-20240101T140000Z.dat (current)
│               └── .metadata.json  # Version metadata
```

### 2. Data Model Extensions (High Impact)

Need to add new data structures:

```rust
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ObjectVersion {
    pub version_id: String,
    pub is_latest: bool,
    pub last_modified: DateTime<Utc>,
    pub etag: String,
    pub size: u64,
    pub is_delete_marker: bool,
    pub storage_path: PathBuf,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ObjectMetadata {
    pub key: String,
    pub versions: Vec<ObjectVersion>,
    pub delete_markers: Vec<ObjectVersion>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct BucketVersioningConfig {
    pub versioning_enabled: bool,
    pub mfa_delete: bool, // For compatibility
}
```

### 3. API Endpoints to Add (Medium Impact)

**New Endpoints**:
- `PUT /{bucket}?versioning` - Enable/disable versioning
- `GET /{bucket}?versioning` - Get versioning status  
- `GET /{bucket}?versions` - List all object versions
- `GET /{bucket}/{key}?versionId={id}` - Get specific version
- `DELETE /{bucket}/{key}?versionId={id}` - Delete specific version

**Enhanced Endpoints**:
- All existing endpoints need `versionId` parameter support
- PUT operations must generate new version IDs
- DELETE operations must create delete markers (unless targeting specific version)

### 4. Modified Core Operations (High Impact)

#### PUT Object Changes
```rust
pub async fn handle_versioned_put(
    config: Extension<Arc<Config>>,
    headers: HeaderMap,
    Path((bucket, key)): Path<(String, String)>,
    bytes: Bytes,
) -> Result<Response, S3AppError> {
    // 1. Check if bucket has versioning enabled
    // 2. Generate new version ID
    // 3. Store file with version-specific path
    // 4. Update metadata with new version
    // 5. Return version ID in response headers
}
```

#### GET Object Changes
```rust
pub async fn handle_versioned_get(
    config: Extension<Arc<Config>>,
    query: Query<GetObjectQuery>,
    Path((bucket, key)): Path<(String, String)>,
) -> Result<Response, S3AppError> {
    // 1. Parse versionId from query params
    // 2. If no versionId, get latest non-delete-marker version
    // 3. Return specific version with version headers
}
```

#### DELETE Object Changes
```rust
pub async fn handle_versioned_delete(
    config: Extension<Arc<Config>>,
    query: Query<DeleteObjectQuery>,
    Path((bucket, key)): Path<(String, String)>,
) -> Result<Response, S3AppError> {
    // 1. If versionId specified, permanently delete that version
    // 2. If no versionId, create delete marker
    // 3. Update metadata accordingly
}
```

### 5. Configuration Changes (Low Impact)

Add to `Config` struct:
```rust
pub struct Config {
    // ... existing fields
    pub versioning_enabled: bool,
    pub max_versions_per_object: Option<u32>,
    pub auto_cleanup_old_versions: bool,
}
```

Update environment variable configuration:
```bash
# Versioning configuration
export FILY_VERSIONING_ENABLED=true
export FILY_MAX_VERSIONS_PER_OBJECT=100  # Optional limit
export FILY_AUTO_CLEANUP_OLD_VERSIONS=false
```

Add to `src/config.rs` loader:
```rust
fn load_versioning_config() -> Result<VersioningConfig> {
    let enabled = env::var("FILY_VERSIONING_ENABLED")
        .map(|v| v.to_lowercase() == "true")
        .unwrap_or(false);
    
    let max_versions = env::var("FILY_MAX_VERSIONS_PER_OBJECT")
        .ok()
        .and_then(|v| v.parse().ok());
    
    let auto_cleanup = env::var("FILY_AUTO_CLEANUP_OLD_VERSIONS")
        .map(|v| v.to_lowercase() == "true")
        .unwrap_or(false);
    
    Ok(VersioningConfig {
        enabled,
        max_versions_per_object: max_versions,
        auto_cleanup_old_versions: auto_cleanup,
    })
}
```

### 6. Implementation Complexity Estimate

#### High Complexity Items
- **Version-aware storage backend** (2-3 weeks)
  - Metadata management system
  - Atomic file operations
  - Version ID generation and validation
  
- **Concurrent operation safety** (1 week)
  - File locking mechanisms
  - Transaction-like operations for metadata
  - Race condition prevention

#### Medium Complexity Items
- **API endpoint modifications** (1 week)
  - Query parameter parsing for version IDs
  - Response header modifications
  - Error handling for version-specific scenarios
  
- **Version listing and querying** (1 week)
  - Efficient version lookup
  - Sorting and filtering versions
  - Pagination support
  
- **Delete marker logic** (3-5 days)
  - Creating delete markers vs permanent deletion
  - Handling version-specific deletes

#### Low Complexity Items
- **Configuration updates** (1-2 days)
  - Config struct modifications
  - Environment variable parsing updates
  
- **Basic version ID generation** (1 day)
  - UUID or timestamp-based IDs
  - Ensuring uniqueness

### 7. Additional Considerations

#### Performance Impact
- **Metadata operations** on every request add overhead
- **Filesystem overhead** with many versions per object
- Need **efficient version lookup** mechanisms
- Consider **indexing** strategies for large numbers of versions

#### Storage Management
- **Disk space grows significantly** with versioning
- Need **cleanup policies** for old versions
- Consider **compression** for older versions
- Implement **storage quotas** per bucket/user

#### Concurrency Issues
- **Race conditions** when updating metadata
- Need **file locking** or atomic operations
- Consider using **SQLite for metadata** instead of JSON files
- **Backup and recovery** strategies for metadata

#### AWS S3 Compatibility
- **Version ID format** must match AWS expectations (typically 32-character strings)
- **Response headers** must include version information (`x-amz-version-id`)
- **Error codes** for version-specific operations
- Support for **MFA delete** (even if no-op implementation)
- **ETags** must be consistent across versions
- **Last-Modified** dates per version

### 8. Recommended Implementation Phases

#### Phase 1: Foundation (2-3 weeks)
- [ ] Design and implement version-aware storage backend
- [ ] Create metadata management system
- [ ] Implement version ID generation
- [ ] Add basic versioning configuration (environment variables)

#### Phase 2: Core Operations (1-2 weeks)
- [ ] Modify PUT object for versioning
- [ ] Update GET object with version support
- [ ] Implement versioned DELETE with delete markers
- [ ] Add version-specific query parameter parsing

#### Phase 3: Versioning Management (1 week)
- [ ] Add bucket versioning configuration endpoints
- [ ] Implement list versions endpoint
- [ ] Add version-specific object operations

#### Phase 4: Advanced Features (1-2 weeks)
- [ ] Implement cleanup policies
- [ ] Add version lifecycle management
- [ ] Performance optimization
- [ ] Comprehensive error handling

#### Phase 5: Testing and Validation (1 week)
- [ ] Unit tests for all versioning functionality
- [ ] Integration tests with AWS CLI
- [ ] Performance benchmarking
- [ ] Documentation updates

## Implementation Challenges

### Technical Challenges
1. **Atomic Operations**: Ensuring metadata and file operations are consistent
2. **Concurrency**: Multiple clients modifying the same object simultaneously
3. **Performance**: Metadata operations on every request
4. **Storage Efficiency**: Managing disk space with many versions

### Design Decisions
1. **Metadata Storage**: JSON files vs embedded database (SQLite)
2. **Version ID Format**: UUID vs timestamp-based
3. **File Organization**: Directory structure for versions
4. **Cleanup Strategy**: Manual vs automatic version cleanup

### Testing Strategy
1. **Unit Tests**: Each versioning operation in isolation
2. **Integration Tests**: End-to-end workflows with AWS CLI
3. **Concurrent Tests**: Multiple clients accessing same objects
4. **Performance Tests**: Large numbers of versions
5. **Compatibility Tests**: AWS S3 API compliance

## Estimated Total Effort

**Complete Implementation**: 6-8 weeks for a full-featured implementation

**Minimum Viable Product**: 4-5 weeks for basic versioning support

This represents a substantial undertaking that essentially requires rewriting the storage layer and most of the core functionality. The current simple file-per-object approach would need to be completely replaced with a more sophisticated version-aware system.

## Alternative Approaches

### Option 1: Metadata Database
Use SQLite or similar embedded database for metadata instead of JSON files:
- **Pros**: Better performance, ACID transactions, complex queries
- **Cons**: Additional dependency, more complex setup

### Option 2: Copy-on-Write Strategy
Keep current file structure but copy files for each version:
- **Pros**: Simpler implementation, familiar file structure
- **Cons**: Higher disk usage, slower writes

### Option 3: Hybrid Approach
Implement basic versioning first, then optimize:
- **Pros**: Faster initial implementation, iterative improvement
- **Cons**: Potential rework, migration complexity

## Conclusion

S3-compatible versioning is a major feature that would significantly enhance Fily's capabilities but requires substantial architectural changes. The implementation should be approached systematically, starting with a solid foundation and building up functionality in phases.

The most critical success factors are:
1. Robust metadata management
2. Atomic operations for consistency
3. Efficient storage and lookup mechanisms
4. Comprehensive testing for AWS compatibility