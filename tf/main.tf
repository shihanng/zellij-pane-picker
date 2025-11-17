provider "github" {
  owner = "shihanng"
}

module "github" {
  source     = "github.com/shihanng/tf-github-repo?ref=v0.1.0"
  repository = "zellij-pane-picker"
}

resource "github_repository" "this" {
  name                   = "zellij-pane-picker"
  description            = "Quickly switch, star, and jump to panes with customizable keyboard shortcuts"
  visibility             = "public"
  delete_branch_on_merge = true
  has_downloads          = true
  has_issues             = true
  has_projects           = true
  vulnerability_alerts   = true
  allow_update_branch    = true
}

terraform {
  required_version = "~> 1.11"

  required_providers {
    github = {
      source  = "integrations/github"
      version = "~> 6.0"
    }
  }

  cloud {
    organization = "shihan"

    workspaces {
      name = "zellij-pane-picker"
    }
  }
}
