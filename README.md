# Issues helper

This is little executable designed to make opening issues easier: type `gli "my shiny issue"` in a git repo, and it will automatically open an issue in the corresponding gitlab repo. For now it only supports one gitlab account, and one github account. Multiple gitlab (and github) accounts are in the roadmap.

## Install

You need [cargo](http://doc.crates.io/) to install *issues-helper*.

    cargo install gli # will install an executable called `gli`
    gli init # inital configuration (gitlab domain, personal access tokens)

## Use

### Open an issue

    gli o "my shiny issue" ["my issue text"] [--open] [--label suggestion]*

`--open` will automatically open the issue page in the browser for further edition.
`--assignee` allows you to assign the issue to a user. For now it only supports one assignee.
`--label` allows you to specify a label when creating an issue. It's a multiple option, so either put it at the end of the command or put `--` before the issue title.

### Open the project page in your browser

    gli b

## List open issues

    gli l

You can optionally add a `--filter open|closed` option to filter issues by state. It only works on gitlab for now, though.

## Requirements

### `origin` remote

For this to work, the project's `origin` remote must look like `git@<domain-name>:<namespace>/<project>.git`,
`git+ssh://git@<domain-name>/<namespace>/<project>.git` or
`https://<domain-name>/<namespace>/<project>.git`
else it wont work.
