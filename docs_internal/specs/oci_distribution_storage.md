# Technical Report: Optimized Storage Model for OCI Distribution

## 1. Technical Architecture: Content-Addressable Storage (CAS)
An effektive OCI registry storage model is built on the principle of **Content-Addressable Storage (CAS)**, where data is retrieved based on a cryptographic hash of its content rather than its location,. This model ensures that every unique blob—whether a filesystem layer, image configuration, or manifest—is stored only once within a **Global Blob Pool**,,.

### The Global Blob Pool
In this domain, every file is keyed by its SHA-256 digest,. If multiple container images share an identical base layer, such as a standard Ubuntu distribution, that layer's bytes are stored only once in the blob store, providing automatic data deduplication,,,.
*   **Path Convention:** All content is typically stored under a structure like `v2/blobs/sha256/<first-two-chars-of-digest>/<full-digest>`,,.

### The Repository Namespace
While the blob pool handles raw data, the **Repository Namespace** manages human-readable metadata, mapping tags (e.g., `v1.0`) to specific manifest digests,,. This metadata layer acts as the "who" and "where" of the registry, while the blob pool is the "what".

## 2. Technical Workflows

### Pushing an Image (Blob-First)
The OCI push workflow is designed to be efficient by uploading data before referencing it in a manifest,.
1.  **Blob Upload:** The client first hashes each layer and the image configuration JSON. It uploads these as individual, opaque blobs,,.
2.  **Existence Check:** Before uploading, the client performs a `HEAD` request to see if the digest already exists; if it does, the upload is skipped to save bandwidth,.
3.  **Manifest Upload:** Once all blobs are stored, the client pushes the **Image Manifest**,. This JSON document "stitches" the blobs together by listing their digests in the correct stack order,,.
4.  **Tagging:** By pushing the manifest to a specific reference, the registry creates a tag pointer in the Repository Namespace,.

### Pulling an Image (Manifest-First)
Pulling is the inverse process, starting with metadata to identify which data chunks are needed.
1.  **Fetch Manifest:** The client requests a manifest by its tag or digest,,.
2.  **Parse Descriptors:** The client reads the manifest to identify the cryptographic digests of the configuration and all filesystem layers,,.
3.  **Download Blobs:** The client fetches each blob individually using its digest from the Global Blob Pool,,.

## 3. Discovery and Search
The OCI specification focuses on standardized content distribution rather than complex searching,.
*   **Lack of Formal Search:** The OCI Distribution Spec does not include a requirement for a full-text search API,. Endpoints like `_catalog` are common but are not standardized across all vendors.
*   **Referrers API (v1.1):** Version 1.1 introduced the **Referrers API** to enable discovery of artifacts (like signatures or SBOMs) that reference a specific manifest without modifying the original image,,.

## 4. Verdict: Indexed Storage and B-Trees
A technical decision must be made regarding the use of indexed storage structures like B-Trees.

### For Blob Retrieval: Unnecessary
For retrieving a specific layer or configuration file, an index is not required. Because the storage is content-addressable, the registry can calculate the exact file path from the digest, allowing for direct retrieval from the blob store without searching,.

### For Metadata Management: Essential
A separate **indexed metadata store** (typically using B-Trees) is critical for managing the Repository Namespace:
1.  **Lexical Tag Ordering:** The specification requires that `tags/list` results be returned in lexical order, which is efficiently handled by B-Tree structures,.
2.  **Referrer Reverse-Lookups:** Finding all artifacts that reference a specific `subject` digest requires a reverse-lookup index to avoid expensive full-system scans.
3.  **Garbage Collection:** To safely delete a blob, the registry must know if any manifests still reference it. An indexed database is necessary to maintain these relational links,.

**Conclusion:** Use a **blob store** for immutable, heavy content and a **relational or key-value index** for metadata, tags, and relationship tracking,.
