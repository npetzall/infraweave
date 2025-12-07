variable "REGISTRY" {}
variable "VERSION" {}

target "reconciler-generic" {
  context = "."
  dockerfile = "reconciler/Dockerfile.generic"
  tags = ["${REGISTRY}/reconciler-generic:${VERSION}"]
  platforms = ["linux/arm64"]
}

target "reconciler-aws" {
  context = "."
  dockerfile = "reconciler/Dockerfile.lambda"
  tags = ["${REGISTRY}/reconciler-aws:${VERSION}"]
  platforms = ["linux/arm64"]
}

group "default" {
  targets = ["gitops-aws"]
}
