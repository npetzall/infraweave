# Build chef stage (base with cargo-chef)
target "chef" {
  context = "."
  dockerfile = "docker/Dockerfile.alpine-builder"
  target = "chef"
}

# Build planner stage (prepares recipe.json from source)
target "planner" {
  context = "."
  dockerfile = "docker/Dockerfile.alpine-builder"
  target = "planner"
  contexts = {
    chef = "target:chef"
  }
}

# Build dependencies stage (compiles Rust dependencies)
target "dependencies" {
  context = "."
  dockerfile = "docker/Dockerfile.alpine-builder"
  target = "dependencies"
  contexts = {
    chef = "target:chef"
    planner = "target:planner"
  }
}

# Build the terraform stage (depends on chef)
target "terraform-stage" {
  context = "."
  dockerfile = "terraform_runner/Dockerfile.terraform"
  target = "terraform"
  contexts = {
    chef = "target:chef"
  }
}

# Build the tofu stage (depends on chef)
target "tofu-stage" {
  context = "."
  dockerfile = "terraform_runner/Dockerfile.tofu"
  target = "tofu"
  contexts = {
    chef = "target:chef"
  }
}

# Build the opa stage (depends on chef)
target "opa-stage" {
  context = "."
  dockerfile = "terraform_runner/Dockerfile.opa"
  target = "opa"
  contexts = {
    chef = "target:chef"
  }
}

# Build the builder stage (depends on dependencies)
target "builder-stage" {
  context = "."
  dockerfile = "terraform_runner/Dockerfile.runner"
  target = "builder"
  contexts = {
    dependencies = "target:dependencies"
  }
}

# Runner using terraform
target "runner-terraform" {
  context = "."
  dockerfile = "terraform_runner/Dockerfile.runner"
  contexts = {
    terraform = "target:terraform-stage"
    opa = "target:opa-stage"
    builder = "target:builder-stage"
  }
  args = {
    REGISTRY_API_HOSTNAME = "registry.terraform.io"
  }
  tags = ["runner:terraform"]
}

# Runner using tofu (map tofu stage to terraform context)
target "runner-tofu" {
  context = "."
  dockerfile = "terraform_runner/Dockerfile.runner"
  contexts = {
    terraform = "target:tofu-stage"  # Map tofu stage to terraform context
    opa = "target:opa-stage"
    builder = "target:builder-stage"
  }
  args = {
    REGISTRY_API_HOSTNAME = "registry.opentofu.org"
  }
  tags = ["runner:tofu"]
}

# Build both runners
group "default" {
  targets = ["runner-terraform", "runner-tofu"]
}

