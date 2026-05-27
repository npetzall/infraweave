# registry-client

OpenTofu/Terraform registry client: download a **known** provider version (per platform, with SHA256SUMS + GPG verification) or a **known** module version (via `X-Terraform-Get` / JSON `location`, then HTTP(S) archive fetch).

See crate docs (`src/lib.rs`) for scope and intentional spec deviations.
