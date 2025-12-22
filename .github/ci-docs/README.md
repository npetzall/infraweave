# CI/CD Workflow Documentation

This directory contains documentation and tooling for understanding and testing the CI/CD workflows used in this repository.

## Overview

The main CI/CD workflow (`main.yml`) orchestrates a series of reusable workflows that handle:
- Version calculation
- Binary builds
- Docker image builds
- Python wheel builds
- Testing
- Release publishing

The workflows are designed to be highly customizable through GitHub repository variables, allowing you to configure which targets to build, which binaries to compile, and where to publish releases without modifying workflow files.

## GitHub Variables

The workflows use GitHub repository variables to customize build and release behavior. These variables are JSON strings that define configuration data. You can set these variables in your repository settings under **Settings → Secrets and variables → Actions → Variables**.

### Available Variables

#### `TARGETS`

Defines the available build targets (platforms and architectures) and their configuration.

**Format:** JSON object mapping target names to their configuration

**Example:**
```json
{
    "linux-amd64": {"rust_target": "x86_64-unknown-linux-gnu", "runner": "ubuntu-latest", "cross": false},
    "linux-amd64-musl": {"rust_target": "x86_64-unknown-linux-musl", "runner": "ubuntu-latest"},
    "linux-arm64": {"rust_target": "aarch64-unknown-linux-gnu", "runner": "ubuntu-latest"},
    "linux-arm64-musl": {"rust_target": "aarch64-unknown-linux-musl", "runner": "ubuntu-latest"},
    "macos-amd64": {"rust_target": "x86_64-apple-darwin", "runner": "macos-latest"},
    "macos-arm64": {"rust_target": "aarch64-apple-darwin", "runner": "macos-latest", "cross": false},
    "windows-amd64": {"rust_target": "x86_64-pc-windows-msvc", "runner": "windows-latest", "cross": false}
}
```

**Fields:**
- `rust_target`: The Rust target triple (e.g., `x86_64-unknown-linux-gnu`)
- `runner`: The GitHub Actions runner to use (e.g., `ubuntu-latest`, `macos-latest`, `windows-latest`)
- `cross`: Optional boolean indicating whether to use `cross` for cross-compilation (defaults to `true` if not specified)

**Used by:**
- `bin.yml` - Binary build workflow
- `wheels.yml` - Python wheel build workflow

**Example file:** `data/targets.example.json`

#### `BINARIES`

Defines which binaries to build and which targets each binary should be built for.

**Format:** JSON array of objects, each containing a binary name and its target list

**Example:**
```json
[
    {
        "bin": "cli",
        "targets": ["linux-amd64", "linux-amd64-musl", "linux-arm64", "linux-arm64-musl", "macos-amd64", "macos-arm64", "windows-amd64"]
    },
    {
        "bin": "gitops",
        "targets": ["linux-amd64", "linux-amd64-musl", "linux-arm64", "linux-arm64-musl"]
    }
]
```

**Fields:**
- `bin`: The name of the binary to build (must match a binary defined in `Cargo.toml`)
- `targets`: Array of target names that must exist in the `TARGETS` variable

**Used by:**
- `bin.yml` - Binary build workflow

**Example file:** `data/binaries.example.json`

#### `PYTHON_WHEELS`

Defines which Python wheel targets to build.

**Format:** JSON array of target names

**Example:**
```json
[
    "linux-amd64",
    "linux-arm64",
    "linux-amd64-musl",
    "linux-arm64-musl",
    "windows-amd64",
    "macos-arm64"
]
```

**Note:** All target names must exist in the `TARGETS` variable.

**Used by:**
- `wheels.yml` - Python wheel build workflow

**Example file:** `data/python_wheels.example.json`

#### `DOCKER_IMAGES`

Defines which Docker images to build and their target platforms.

**Format:** JSON array of objects, each containing an image name and its platforms

**Example:**
```json
[
    {
        "name": "cli",
        "platforms": ["linux/amd64", "linux/arm64"]
    },
    {
        "name": "gitops",
        "platforms": ["linux/amd64", "linux/arm64"]
    }
]
```

**Fields:**
- `name`: The name of the Docker image (must match a binary name from `BINARIES`)
- `platforms`: Array of Docker platform identifiers (e.g., `linux/amd64`, `linux/arm64`)

**Used by:**
- `docker.yml` - Docker image build workflow
- `release.yml` - Release workflow (for publishing images)

**Example file:** `data/docker_images.example.json`

#### `RELEASE_REGISTRIES`

Defines Docker registries where images should be published during releases.

**Format:** JSON array of registry configuration objects

**Example:**
```json
[
    {
        "name": "Docker Hub",
        "registry": "docker.io",
        "repository": "infraweave",
        "auth_type": "login",
        "user_secret": "DOCKER_USER",
        "password_secret": "DOCKER_PASSWORD"
    },
    {
        "name": "Quay.io",
        "registry": "quay.io",
        "repository": "infraweave",
        "auth_type": "login",
        "user_secret": "QUAY_USER",
        "password_secret": "QUAY_PASSWORD"
    },
    {
        "name": "AWS ECR Public",
        "registry": "public.ecr.aws",
        "repository": "infraweave",
        "auth_type": "aws",
        "account_secret": "AWS_ACCOUNT_ID",
        "role_secret": "ECR_PUBLIC_PUSH_ROLE",
        "aws_region_secret": "ECR_PUBLIC_AWS_REGION"
    }
]
```

**Fields:**
- `name`: Display name for the registry
- `registry`: The registry hostname (e.g., `docker.io`, `quay.io`, `public.ecr.aws`)
- `repository`: The repository name within the registry
- `auth_type`: Authentication method - either `"login"` or `"aws"`
- For `auth_type: "login"`:
  - `user_secret`: Name of the GitHub secret containing the username
  - `password_secret`: Name of the GitHub secret containing the password
- For `auth_type: "aws"`:
  - `account_secret`: Name of the GitHub secret containing the AWS account ID
  - `role_secret`: Name of the GitHub secret containing the IAM role name
  - `aws_region_secret`: Name of the GitHub secret containing the AWS region

**Used by:**
- `release.yml` - Release workflow (publish-images job)

**Example file:** `data/release_registries.example.json`

#### `DOCKER_IMAGE_MIRROR`

Defines external Docker images to mirror into GitHub Container Registry (GHCR).

**Format:** JSON array of image mapping objects

**Example:**
```json
[
    {
        "from": "public.ecr.aws/lambda/python:3.11",
        "to": "lambda-python:3.11"
    },
    {
        "from": "mcr.microsoft.com/azure-functions/python:4.0",
        "to": "azure-functions-python:4.0"
    }
]
```

**Fields:**
- `from`: Source image reference (full image name with registry)
- `to`: Destination image name (will be pushed to `ghcr.io/<repository>/<to>`)

**Used by:**
- `mirror-docker-images.yml` - Docker image mirroring workflow

**Example file:** `data/docker_image_mirror.example.json`

## Customizing the Workflow

### Adding a New Build Target

1. Add the target to the `TARGETS` variable:
   ```json
   {
       "my-new-target": {
           "rust_target": "x86_64-unknown-linux-gnu",
           "runner": "ubuntu-latest",
           "cross": true
       }
   }
   ```

2. Reference the target in `BINARIES` or `PYTHON_WHEELS` as needed.

### Adding a New Binary

1. Ensure the binary is defined in your `Cargo.toml`
2. Add an entry to the `BINARIES` variable:
   ```json
   {
       "bin": "my-new-binary",
       "targets": ["linux-amd64", "linux-arm64"]
   }
   ```

3. Optionally add a corresponding entry to `DOCKER_IMAGES` if you want to build a Docker image for it.

### Adding a New Release Registry

1. Add an entry to the `RELEASE_REGISTRIES` variable with the appropriate authentication configuration
2. Ensure the required secrets are configured in your repository (Settings → Secrets and variables → Actions → Secrets)

### Disabling Features

- **Disable Docker image publishing:** Set `RELEASE_REGISTRIES` to an empty array `[]` or leave it unset
- **Disable specific binaries:** Remove entries from the `BINARIES` variable
- **Disable specific targets:** Remove entries from the `TARGETS` variable (and remove references from `BINARIES` and `PYTHON_WHEELS`)

## Testing Scripts Locally

The wrapper scripts in this directory allow you to test the GitHub Actions scripts locally by simulating the GitHub Actions environment.

### Available Wrapper Scripts

- **`run-bin-scripts.sh`** - Tests binary build matrix scripts
- **`run-wheels-scripts.sh`** - Tests Python wheel build matrix scripts
- **`run-calculate-version.sh`** - Tests version calculation script
- **`run-release-generate-notes.sh`** - Tests release notes generation script

### How They Work

The wrapper scripts:
1. Create temporary files for `GITHUB_OUTPUT` and `GITHUB_STEP_SUMMARY` (used by GitHub Actions)
2. Load configuration from example JSON files in `data/` (or prompt for custom paths)
3. Set up environment variables that the scripts expect
4. Run the actual scripts from `.github/scripts/`
5. Display the output and cleanup temporary files

### Usage Examples

#### Testing Binary Build Scripts

```bash
cd .github/ci-docs
./run-bin-scripts.sh
```

The script will:
- Prompt for paths to `targets.example.json` and `binaries.example.json` (with defaults)
- Run `bin_validate-targets.sh` to validate the configuration
- Run `bin_setup-build-matrix.sh` to generate the build matrix
- Display the `GITHUB_OUTPUT` and `GITHUB_STEP_SUMMARY` contents

#### Testing Python Wheel Scripts

```bash
cd .github/ci-docs
./run-wheels-scripts.sh
```

Similar to the binary scripts, but uses `targets.example.json` and `python_wheels.example.json`.

#### Testing Version Calculation

```bash
cd .github/ci-docs
./run-calculate-version.sh
```

The script will prompt for:
- Whether this is a pull request
- PR number (if applicable)
- Current branch
- Default branch
- Whether this is a release build

**Note:** On macOS, this script requires GNU grep (`ggrep`). Install it with:
```bash
brew install grep
```

#### Testing Release Notes Generation

```bash
cd .github/ci-docs
./run-release-generate-notes.sh
```

The script will prompt for a version number and then generate release notes based on conventional commits.

**Note:** On macOS, this script also requires GNU grep (`ggrep`).

### Customizing Test Data

You can create your own JSON configuration files and provide their paths when prompted by the wrapper scripts. This allows you to test different configurations without modifying the example files.

For example, to test with a custom binaries configuration:

```bash
cd .github/ci-docs
./run-bin-scripts.sh
# When prompted, provide path to your custom binaries.json file
```

## Docker Image Mirroring Workflow

The `mirror-docker-images.yml` workflow allows you to mirror external Docker images into GitHub Container Registry (GHCR). This is useful for caching frequently-used base images or third-party images that your workflows depend on.

### How It Works

The workflow:
1. Reads the `DOCKER_IMAGE_MIRROR` variable to determine which images to mirror
2. For each image mapping, it:
   - Pulls the source image from its original registry
   - Tags it for GHCR (as `ghcr.io/<repository>/<to>`)
   - Pushes it to GHCR

### Configuration

Configure the images to mirror by setting the `DOCKER_IMAGE_MIRROR` GitHub variable (see the [DOCKER_IMAGE_MIRROR](#docker_image_mirror) variable documentation above).

### Running the Workflow

The workflow is manually triggered via `workflow_dispatch`:

1. Go to **Actions** tab in your GitHub repository
2. Select **Mirror Docker Images to GHCR** from the workflow list
3. Click **Run workflow**
4. The workflow will process all images defined in `DOCKER_IMAGE_MIRROR`

### Authentication

The workflow supports optional Docker Hub authentication to avoid rate limits:

- **Without authentication:** Public images can be pulled, but you may hit rate limits
- **With authentication:** Set the following secrets to use authenticated pulls:
  - `DOCKERHUB_USERNAME` - Your Docker Hub username
  - `DOCKERHUB_TOKEN` - Your Docker Hub access token or password

If `DOCKERHUB_USERNAME` is not set, the workflow will skip Docker Hub authentication and attempt to pull images anonymously.

**Note:** The workflow always authenticates to GHCR using the `GITHUB_TOKEN` secret (automatically provided by GitHub Actions).

### Use Cases

- **Avoiding rate limits for test images:** Mirror Docker images used in test workflows to avoid hitting Docker Hub rate limits when running multiple test jobs. By pulling from GHCR instead of external registries, you can run more concurrent test jobs without authentication concerns.

### Example Configuration

To mirror a Python base image and an Azure Functions image:

```json
[
    {
        "from": "public.ecr.aws/lambda/python:3.11",
        "to": "lambda-python:3.11"
    },
    {
        "from": "mcr.microsoft.com/azure-functions/python:4.0",
        "to": "azure-functions-python:4.0"
    }
]
```

After running the workflow, these images will be available at:
- `ghcr.io/<your-repo>/lambda-python:3.11`
- `ghcr.io/<your-repo>/azure-functions-python:4.0`

### Using Mirrored Images in Integration Tests

The integration test suite (`integration-tests/tests/utils.rs`) automatically uses mirrored images when the `DOCKER_IMAGE_MIRROR` environment variable is set. This allows integration tests to pull images from GHCR instead of external registries, avoiding rate limits.

#### How It Works

The `get_image_name()` function in `integration-tests/tests/utils.rs`:
1. Checks for the `DOCKER_IMAGE_MIRROR` environment variable
2. If set, maps original image names to their mirrored equivalents
3. Constructs the full image path as `{DOCKER_IMAGE_MIRROR}/{mirrored_name}:{tag}`

The following images are automatically mapped when `DOCKER_IMAGE_MIRROR` is set:
- `public.ecr.aws/lambda/python` → `lambda-python`
- `mcr.microsoft.com/azure-functions/python` → `azure-functions-python`
- `mcr.microsoft.com/cosmosdb/linux/azure-cosmos-emulator` → `azure-cosmos-emulator`
- `minio/minio` → `minio`
- `mcr.microsoft.com/azure-storage/azurite` → `azurite`
- `amazon/dynamodb-local` → `dynamodb-local`

#### Setting the Environment Variable in Workflows

To use mirrored images in your integration test workflow, set the `DOCKER_IMAGE_MIRROR` environment variable to your GHCR registry path:

```yaml
- name: Run integration tests for ${{ matrix.provider }}
  env:
    DOCKER_IMAGE_MIRROR: ghcr.io/${{ github.repository }}
  run: |
    make ${{ matrix.provider }}-integration-tests
```

**Note:** The environment variable should be set to the registry prefix (e.g., `ghcr.io/owner/repo`), not the full image path. The test code will append the mirrored image name and tag automatically.

#### Running Locally

When running integration tests locally, you can optionally set the environment variable:

```bash
DOCKER_IMAGE_MIRROR=ghcr.io/owner/repo make aws-integration-tests
```

If `DOCKER_IMAGE_MIRROR` is not set, the tests will use the original images from their source registries.

## Example Data Files

The `data/` directory contains example JSON files that demonstrate the expected format for each variable:

- `targets.example.json` - Example `TARGETS` configuration
- `binaries.example.json` - Example `BINARIES` configuration
- `python_wheels.example.json` - Example `PYTHON_WHEELS` configuration
- `docker_images.example.json` - Example `DOCKER_IMAGES` configuration
- `release_registries.example.json` - Example `RELEASE_REGISTRIES` configuration
- `docker_image_mirror.example.json` - Example `DOCKER_IMAGE_MIRROR` configuration

These files can be used as templates when configuring your GitHub repository variables, or as test data when running the wrapper scripts locally.
