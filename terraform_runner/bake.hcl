variable "REGISTRY" {}
variable "VERSION" {}

# Build the terraform stage (depends on chef)
target "terraform-stage" {
  context = "."
  dockerfile = "terraform_runner/Dockerfile.terraform"
  target = "terraform"
  platforms = ["linux/arm64"]
}

# Build the tofu stage (depends on chef)
target "tofu-stage" {
  context = "."
  dockerfile = "terraform_runner/Dockerfile.tofu"
  target = "tofu"
  platforms = ["linux/arm64"]
}

# Build the opa stage (depends on chef)
target "opa-stage" {
  context = "."
  dockerfile = "terraform_runner/Dockerfile.opa"
  target = "opa"
  platforms = ["linux/arm64"]
}

# Runner using terraform
target "runner-terraform" {
  context = "."
  dockerfile = "terraform_runner/Dockerfile.runner"
  contexts = {
    terraform = "target:terraform-stage"
    opa = "target:opa-stage"
  }
  args = {
    REGISTRY_API_HOSTNAME = "registry.terraform.io"
  }
  tags = ["${REGISTRY}/runner:${VERSION}-terraform"]
  platforms = ["linux/arm64"]
}

# Runner using tofu (map tofu stage to terraform context)
target "runner-tofu" {
  context = "."
  dockerfile = "terraform_runner/Dockerfile.runner"
  contexts = {
    terraform = "target:tofu-stage"  # Map tofu stage to terraform context
    opa = "target:opa-stage"
  }
  args = {
    REGISTRY_API_HOSTNAME = "registry.opentofu.org"
  }
  tags = ["${REGISTRY}/runner:${VERSION}-tofu"]
  platforms = ["linux/arm64"]
}

# Build both runners
group "default" {
  targets = ["runner-terraform", "runner-tofu"]
}

