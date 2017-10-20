# Gitlab helper

This is little executable designed to make opening issues easier: type `gli "my shiny issue"` in a git repo, and it will automatically open an issue in the corresponding gitlab repo.

## Use

### Open an issue

    gli "my shiny issue"

### Open the project page in your browser

    gli

## Install

Clone this repo, run `cargo install`. You'll have `gli` in your path.

## Requirements

### Gitlab access token

You need to put access token in `$XDG_CONFIG_HOME/gitlab-CHANGEME` (by default it's `$HOME/.config/gitlab-CHANGEME`).
You can create access tokens on gitlab: <https://gitlab.clever-cloud.com/profile/personal_access_tokens>

### `origin` remote

For this to work, the project's `origin` remote must look like `git@CHANGEME:<namespace>/<project>.git`, else it wont work.

## ToDo

- Quick open of issue page / prs page