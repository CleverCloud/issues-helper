# Issues helper

This is little executable designed to make opening issues easier: type `gli "my shiny issue"` in a git repo, and it will automatically open an issue in the corresponding gitlab repo. For now it only supports one gitlab account, but multiple accounts and github support are planned.

## Install

You need [cargo](http://doc.crates.io/) to install *issues-helper*.

    cargo install issues-helper # will install an executable called `gli`
    gli init # inital configuration (gitlab domain, personal access token)

## Use

### Open an issue

    gli o "my shiny issue" ["my issue text"] [--open] [--label suggestion]*

`--open` will automatically open the issue page in the browser for further edition.
`--label` allows you to specify a label when creating an issue. It's a multiple option, so either put it at the end of the command or put `--` before the issue title.

### Open the project page in your browser

    gli b

## Requirements

### `origin` remote

For this to work, the project's `origin` remote must look like `git@<domain-name>:<namespace>/<project>.git`,
`git+ssh://<domain-name>/<namespace>/<project>.git` or
`https://<domain-name>/<namespace>/<project>.git`
else it wont work.
