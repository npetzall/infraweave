# OpenTofu Registry Protocols: Technical API Specifications Report

## Remote Service Discovery Protocol Specification

This specification defines the entry point for OpenTofu-native services. Clients use this protocol to map a user-facing hostname to the base URLs of specific service APIs. Architecturally, this endpoint serves as the "source of truth" for the registry's capabilities.

Operational Constraints:

* Security: This endpoint **must** be served over **HTTPS**. While port `443` is standard, non-standard ports (e.g., `:8443`) are permitted for development, provided they implement **HTTPS**.
* Hostname Normalization: Hostnames **must** be normalized via the Unicode Nameprep algorithm (lowercase conversion and precomposed diacritics). **Punycode form is strictly prohibited**.
* URL Resolution: Relative URLs provided in the response **must** be resolved against the final discovery URL reached after following any HTTP redirects.
```yaml
openapi: 3.1.0
info:
  title: OpenTofu Remote Service Discovery API
  version: 1.0.0
  description: |
    The entry point for discovering OpenTofu-native services. 
    Discovery begins by forming a URL using the hostname with the https: scheme 
    and the fixed path /.well-known/terraform.json.
paths:
  /.well-known/terraform.json:
    get:
      summary: Discover service endpoints
      description: |
        Returns a mapping of service identifiers to their respective base URLs. 
        If this request fails (non-200 status, invalid JSON, or non-application/json media type), 
        OpenTofu considers the host to not support native services. 
        Credentials from a credentials_helper or environment variables are included in this request.
      responses:
        '200':
          description: A JSON object containing service identifiers.
          content:
            application/json:
              schema:
                type: object
                properties:
                  login.v1:
                    type: string
                    description: Base URL for the login protocol version 1.
                  modules.v1:
                    type: string
                    description: Base URL for the module registry API version 1.
                  providers.v1:
                    type: string
                    description: Base URL for the provider registry API version 1.
                required:
                  - login.v1
                  - modules.v1
                  - providers.v1
                additionalProperties:
                  type: string
              example:
                login.v1: "/api/login/"
                modules.v1: "/v1/modules/"
                providers.v1: "/v1/providers/"
```


--------------------------------------------------------------------------------


## Provider Registry Protocol Specification

The Provider Registry protocol facilitates version discovery and the retrieval of distribution packages for specific target platforms.
```yaml
openapi: 3.1.0
info:
  title: OpenTofu Provider Registry API
  version: 1.0.0
paths:
  /{namespace}/{type}/versions:
    get:
      summary: List Available Versions
      description: Returns all available versions for a provider.
      parameters:
        - name: namespace
          in: path
          required: true
          schema:
            type: string
        - name: type
          in: path
          required: true
          schema:
            type: string
      responses:
        '200':
          description: A list of available versions and supported platforms.
          content:
            application/json:
              schema:
                type: object
                properties:
                  versions:
                    type: array
                    items:
                      type: object
                      properties:
                        version:
                          type: string
                          description: Semantic Versioning 2.0 string.
                        protocols:
                          type: array
                          items:
                            type: string
                          description: |
                            Array of supported MAJOR.MINOR protocol versions. 
                            Each major version must appear only once, representing 
                            the highest minor version supported for that major.
                        platforms:
                          type: array
                          items:
                            type: object
                            properties:
                              os:
                                type: string
                              arch:
                                type: string
  /{namespace}/{type}/{version}/download/{os}/{arch}:
    get:
      summary: Find a Provider Package
      description: Returns download metadata for a specific provider platform package.
      parameters:
        - name: namespace
          in: path
          required: true
          schema:
            type: string
        - name: type
          in: path
          required: true
          schema:
            type: string
        - name: version
          in: path
          required: true
          schema:
            type: string
        - name: os
          in: path
          required: true
          schema:
            type: string
        - name: arch
          in: path
          required: true
          schema:
            type: string
      responses:
        '200':
          description: Metadata for downloading a specific provider package.
          content:
            application/json:
              schema:
                type: object
                required:
                  - protocols
                  - os
                  - arch
                  - filename
                  - download_url
                  - shasums_url
                  - shasums_signature_url
                  - shasum
                  - signing_keys
                properties:
                  protocols:
                    type: array
                    items:
                      type: string
                    description: Highest minor version supported for each major version.
                  os:
                    type: string
                  arch:
                    type: string
                  filename:
                    type: string
                  download_url:
                    type: string
                  shasums_url:
                    type: string
                  shasums_signature_url:
                    type: string
                  shasum:
                    type: string
                  signing_keys:
                    $ref: '#/components/schemas/signing_keys'
                  packages:
                    type: object
                    description: |
                      If included, metadata for ALL platforms for this provider version 
                      must be provided to support dependency locking.
                    properties:
                      hashes:
                        type: array
                        items:
                          type: string
                        description: |
                          List of valid hashes. Must include at least one 'zh:' 
                          prefixed hash matching the shasum property.
                      package_size:
                        type: integer
components:
  schemas:
    signing_keys:
      type: object
      required:
        - gpg_public_keys
      properties:
        gpg_public_keys:
          type: array
          minItems: 1
          items:
            type: object
            required:
              - key_id
              - ascii_armor
            properties:
              key_id:
                type: string
                description: Uppercase hexadecimal GPG key ID.
              ascii_armor:
                type: string
                description: ASCII-armored GPG public key.
```


--------------------------------------------------------------------------------


## Module Registry Protocol Specification

This protocol allows the OpenTofu CLI to resolve module versions and source locations.
```yaml
openapi: 3.1.0
info:
  title: OpenTofu Module Registry API
  version: 1.0.0
paths:
  /{namespace}/{name}/{system}/versions:
    get:
      summary: List Available Versions
      parameters:
        - name: namespace
          in: path
          required: true
          schema:
            type: string
        - name: name
          in: path
          required: true
          schema:
            type: string
        - name: system
          in: path
          required: true
          schema:
            type: string
      responses:
        '200':
          description: A list of available versions for the module.
          content:
            application/json:
              schema:
                type: object
                properties:
                  modules:
                    type: array
                    description: |
                      The requested module MUST be the first element in this array 
                      to ensure forward compatibility with CLI expectations.
                    items:
                      type: object
                      properties:
                        versions:
                          type: array
                          items:
                            type: object
                            properties:
                              version:
                                type: string
                                description: SemVer 2.0 compatible string.
  /{namespace}/{name}/{system}/{version}/download:
    get:
      summary: Download Source Code
      description: |
        Retrieves the module source location. The primary mechanism is the 
        X-Terraform-Get header. A JSON body with a 'location' field is a valid fallback.
      parameters:
        - name: namespace
          in: path
          required: true
          schema:
            type: string
        - name: name
          in: path
          required: true
          schema:
            type: string
        - name: system
          in: path
          required: true
          schema:
            type: string
        - name: version
          in: path
          required: true
          schema:
            type: string
      responses:
        '204':
          description: |
            Recommended success response. The body is empty, and the source location 
            is provided in the X-Terraform-Get header.
          headers:
            X-Terraform-Get:
              description: The source location for the module download.
              schema:
                type: string
        '200':
          description: |
            Success response utilizing a JSON body fallback. Note that the 
            X-Terraform-Get header takes precedence if both are present.
          headers:
            X-Terraform-Get:
              schema:
                type: string
          content:
            application/json:
              schema:
                type: object
                properties:
                  location:
                    type: string
                    description: Fallback source location URL.
```


--------------------------------------------------------------------------------


## Technical Implementation Requirements

Registry implementers must adhere to the following mandatory constraints to ensure compatibility with OpenTofu:

* Protocol Security: All registry interactions **must** occur over **HTTPS**. OpenTofu will terminate the process if a discovery endpoint returns a non-200 status, invalid JSON, or a media type other than application/json.
* Hostname Normalization & Punycode: User-facing hostnames must be normalized using Unicode Nameprep. Implementations **must not** use Punycode; OpenTofu expects hostnames in their Unicode form.
* Versioning Standards: Both providers and modules must **strictly** use **Semantic Versioning 2.0 (SemVer)**.
* Authentication Logic: If credentials (e.g., via credentials_helper or host-specific environment variables) are configured for a hostname, OpenTofu will include them in the headers for the discovery request and subsequent API calls.
* Discovery Fallbacks: If discovery fails at `/.well-known/terraform.json`, the CLI assumes the host does not support native protocols and may attempt to fall back to other download methods (like Git).
* Response Formatting: Every response **must** utilize the `application/json` media type. For the module download endpoint, a `204 No Content` response is preferred when the `X-Terraform-Get` header is used.


--------------------------------------------------------------------------------


## Data Model Reference Table

| Entity | Key Properties | Protocol Context |
| --- | --- | --- |
| GPG Public Key | key_id, ascii_armor | Provider Registry (Package Signing) |
| Platform | os, arch | Provider Registry (List/Download) |
| Service Discovery Document | login.v1, modules.v1, providers.v1 | Remote Service Discovery |
| Module Address | hostname/namespace/name/system | Module Registry Protocol |
| Provider Package | filename, download_url, shasum | Provider Registry (Download) |
| Module Version | version | Module Registry (Listing) |
