variable "REGISTRY" {}
variable "VERSION" {}

# Runner using terraform
target "cli" {
  context = "."
  dockerfile = "cli/Dockerfile"
  tags = ["${REGISTRY}/cli:${VERSION}"]
  platforms = ["linux/arm64"]
}

# Build both runners
group "default" {
  targets = ["cli"]
}

