variable "REGISTRY" {}
variable "VERSION" {}

target "cli" {
  context = "."
  dockerfile = "cli/Dockerfile"
  tags = ["${REGISTRY}/cli:${VERSION}"]
  platforms = ["linux/arm64"]
}

group "default" {
  targets = ["cli"]
}

