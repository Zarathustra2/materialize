---
name: "Internal: release checklist"
about: >
  A tracking issue for a new release of Materialize. Contributor use only.
---

## Current status: 🚢 Progressing 🚢

<!--
## Current status: 🛑 Blocked 🛑
## Current status: 🚀 [Released](https://github.com/MaterializeInc/materialize/releases/tag/vX.Y.Z) 🚀
-->

## Announce the imminent release internally

- [x] Create this release issue. Determine whether this is a feature release or
  a patch release by checking the [milestones
  page](https://github.com/MaterializeInc/materialize/milestones) -- if the date
  is correct for the next feature release double check with the PMs in the
  #release slack channel and use that version, otherwise it's a patch release.
- [ ] Check for open blocking issues:
  - [ ] For any release, check if there are any open [`M-release-blocker`][rel-blockers]
    issues or PRs
  - [ ] If this is a major or minor release (any X.Y release) check if there are open
    issue or PRs with the [`M-milestone-blocker`][blocker-search] label, include them in
    a list here, or state that there are none:

  > unknown number of milestone blockers or release blockers

[rel-blockers]: https://github.com/MaterializeInc/materialize/issues?q=is%3Aopen+label%3AM-release-blocker
[blocker-search]: https://github.com/MaterializeInc/materialize/issues?q=is%3Aopen+label%3AM-milestone-blocker

## Release candidate

A release candidate is the Materialize codebase at a given commit, tested for
production readiness.

### Create the release candidate

- [ ] Choose the commit for the release. Unless there are open release blockers above,
  this should just be the head commit of the main branch. Checkout that commit.

- [ ] Create a new release candidate, specifying what kind of release this
  should be (see `--help` for all the release types):

  ```shell
  bin/mkrelease new-rc biweekly
  ```

- [ ] Update the [release notes][] to include a new "unreleased" version so that new
  changes don't get advertised as being part of this release as folks add notes.

  > **NOTE:** the next "unreleased" version should always be a patch-level change, even if the next
  > release is currently anticipated to be a minor or major version bump.
  >
  > For example, if the version being released is `v0.5.9`, `<NEXT_VERSION>` should always be
  > `0.5.10`, not `0.6.0`, even if we expect that to be the true next version.
  >
  > The rationale is that minor (and major) versions are feature releases and may slip. If folks
  > are using a development release we expect that they will be less surprised to go from
  > `v0.5.10-dev` -> `v0.6.0` when they upgrade than upgrading and going from `v0.6.0-dev` ->
  > `v0.5.10`.

  - Related to the above note, **if this is a feature release** (i.e. the product team has
    decided that we are bumping the major or minor version) you must also update two items because
    the highest unreleased version will be incorrect. It will be a patch version (i.e. the `Y` in
    `W.X.Y`) instead of a major or minor version change (`W` or `X`), update the release notes to
    reflect the version that is actually being released:
    - [ ] The [release notes][]
    - [ ] Some recent migration versions in `src/coord/src/catalog/migrate.rs`

- [ ] Incorporate this tag into `main`'s history by preparing dev on top of it.

  Without checking out `main`, perform:

  ```shell
  bin/mkrelease incorporate
  ```

  Open a PR with this change, and land it.

  If you have [GitHub cli][gh] this is as easy as typing the following command
  and following some prompts:

  ```console
  $ gh pr create --web
  ```

- [ ] Collect all PRs in this release using the list-prs command and create a slack post in
  `#release` with the output:

  ```shell
  bin/mkrelease list-prs
  ```

- [ ] Review all PRs in the release that were not reviewed by another engineer
  prior to being merged into main.

[gh]: https://cli.github.com/

### Review Release Notes

Release notes should be updated by engineers as they merge PRs. The release notes
team is responsible for reviewing release notes and the release announcement before
a release is published.

- [ ] Post the following message to the `#release` channel in slack, modifying the link to the issue to point to this one:

  > @relnotes-team the release is in progress, now's a great time to verify or
  > prepare the release notes and any announcement posts.
  > * release notes: https://github.com/MaterializeInc/materialize/blob/main/doc/user/content/release-notes.md
  > * release issue: https://github.com/MaterializeInc/materialize/issues/

### Test the release candidate

Cloud engineers run many of the steps in this section, their steps are prefixed with **cloud
engineer**.

To run the required load tests on the release candidate tag, Materialize employees
can follow [these instructions for running semi-automatic load tests][load-instr]
in the infrastructure repository. All of these tests can be run in parallel.

[load-instr]: https://github.com/MaterializeInc/infra/blob/main/doc/starting-a-load-test.md

- [ ] Kick off a [full SQL logic test run](https://buildkite.com/materialize/sql-logic-tests)
  by clicking the 'New Build' button. For the values requested, use the following:

    - Message: Leave blank
    - Commit - Use the default `HEAD`
    - Branch - Enter the release candidate tag (i.e. `v0.4.2-rc1`)

  You can continue on to the next step, but remember to verify that this test passes. Link to it
  here, and don't check this off until you've verified that it passed:

  - [ ] "link to test run"

- [ ] Wait for the docker build of the tag to complete and then start the load tests according to
  [these instructions][load-instr], using your release tag as the `git-ref` value for the release
  benchmarks. You can use [This commit][] as an example to follow.

[This commit]: https://github.com/MaterializeInc/infra/commit/fd7f594d6f9fb2fda3a604f21b730f8d401fe81c

- [ ] Find the load tests in https://grafana.i.mtrlz.dev/d/materialize-overview, and link to them
  in #release, validating that data is present. Note that the default view of that dashboard is of
  a full day, so it may look like the test started and aborted suddenly:

  - [ ] chbench
  - [ ] billing-demo
  - [ ] perf-kinesis

- [ ] **cloud engineer** Let the tests run for at least 24 hours, with the following success
  criteria:

  - [ ] chbench: The ingest rate should not be slower than the previous release.
  - [ ] billing-demo: The container should run and finish without error. You can get the exit code
    from Docker:

    ```bash
    docker ps -a | grep -E 'STATUS|materialized|billing-demo'
    ```

    * `materialized` should be `up` (although see #4753 for the current breakage)
    * `billing-demo` should be `Exited (0)`
    * Note: it is not necessary to verify the graphs of this test if the above criteria are
      met. The graph output is dissimilar to the other tests, and can only be compared to historical
      runs posted in the #release Slack channel.

  - [ ] perf-kinesis: The "Time behind external source" dashboard panel in Grafana should
    have remained at 0ms or similar for the entirety of the run.

- [ ] Let the chaos test run for 24 hours. The test's `chaos_run` container should complete with a
  `0` exit code. To check this, SSH into the EC2 instance running the chaos test and run `docker ps
  -a`. You can ssh in using our [teleport cluster][], the chaos test has a `purpose=chaos` label.

- [ ] Remove all load test machines, [documented here][load-instr].

[teleport cluster]: https://tsh.i.mtrlz.dev/cluster/tsh/nodes

## Final release

### Create Git tag

- [ ] Create the final release based on the final RC tag. For example, if there
  was only one RC, then the final RC tag would be `-rc1`.

  ```shell
  $ bin/mkrelease finish
  ```

  That will create a new branch named `continue-<version>`, the exact name will
  be output as the last line of the script.

  Open a PR with that branch, and land it. This is possible using [gh] from the
  terminal: `gh pr create --web`

### Verify the Release Build and Deploy

- [ ] Find your build in buildkite, for example
  https://buildkite.com/materialize/tests/builds?branch=v0.4.3

  Wait for the completion of the "Deploy", your tag will be listed as the branch at:
  https://buildkite.com/materialize/deploy/builds

  **NOTE:** the deploy step will appear green in the "tests" page before it is actually run,
  because the tests only mark that the async job got kicked off, so check that second link.

### Create Homebrew bottle and update tap

- [ ] Follow the instructions in [MaterializeInc/homebrew-materialize's
  CONTRIBUTING.md][homebrew-guide].

### Verify the Debian package and Docker images


- Run the following commands on a Docker-enabled machine and verify you see
  the correct version.

  - [ ]
    ```shell
    docker run --rm -i ubuntu:bionic <<EOF
    apt-get update && apt-get install -y gpg ca-certificates
    apt-key adv --keyserver keyserver.ubuntu.com --recv-keys 79DEC5E1B7AE7694
    echo "deb http://apt.materialize.com/ generic main" > /etc/apt/sources.list.d/materialize.list
    apt-get update && apt-get install -y materialized
    materialized --version
    EOF
    ```
  - [ ] `docker pull materialize/materialized:latest && docker run --rm materialize/materialized:latest --version`
  - [ ] substitute in the correct version in this one: `docker run --rm materialize/materialized:v0.X.Y --version`

[bintray]: https://bintray.com/beta/#/materialize/apt/materialized

### Open a PR on the cloud repo enabling the new version

- [ ] Issue a PR to the cloud repo to allow the released version following [the instructions][].
- [ ] After that PR has been merged, a PR suggesting a merge from `main` -> `production` will be
  automatically created and assigned to you. Request somebody on the
  [@MaterializeInc/cloud-deployers][deployers] team review it; once approved, merge the PR and it
  will be automatically deployed to production.

[the instructions]: https://github.com/MaterializeInc/cloud/blob/main/doc/misc.md#updating-to-a-new-materialize-release
[deployers]: https://github.com/orgs/MaterializeInc/teams/cloud-deployers/members

### Convert the GitHub Tag into a GitHub Release

- [ ] Go to [the GitHub releases][releases] page and find the tag that you
  created as part of the "Final Release" step.

  - Click "Edit" on the top right
  - Leave the name blank, so that the tag's version is used as the name.
  - Use the following contents as the description, updating the version
    number appropriately:
    ```markdown
    * [Release notes](https://materialize.com/docs/release-notes/#v0.#.#)
    * [Binary tarballs](https://materialize.com/docs/versions/)
    * [Installation instructions](https://materialize.com/docs/install/)
    * [Documentation](https://materialize.com/docs/)
    ```
    You do not need to manually add assets to the release, they will be included
    by GitHub automatically.

### Update the main branch for the next version

- [ ] On `main`, verify the various files that must be up to date:

  - Ensure that the [release notes] are up to date, including the current version.

  - Ensure that [`doc/user/config.toml`] has the correct version, as updated in the "Final
    Release / create git tag" step

  - Ping `#release` again:

    > @relnotes-team the release is complete! we can post to the community slack
    > channel and publish any appropriate blog posts

  - Post a link to the release tag in the `#general` channel, something like the
    following, substituting in the correct version for the X and Y:

    > `🎉🤘 Release v0.X.Y is complete! https://github.com/MaterializeInc/materialize/releases/tag/v0.X.Y 🤘🎉`

## Finish

- (optional) Update the current status at the top of this issue.
- [ ] If this release ran over a week late, update the repeating slack reminder
  in the `release` channel.

  you can view and delete existing reminders by typing `/remind list` in
  `#release`

  I used the following to create the repeating alert, just modify the start date
  to the correct date:

  > /remind #release "@release-team start the next release" on June 21st every 2 weeks

- Close this issue.

[`doc/user/config.toml`]: https://github.com/MaterializeInc/materialize/blob/main/doc/user/config.toml
[`LICENSE`]: https://github.com/MaterializeInc/materialize/tree/main/LICENSE
[`src/materialized/Cargo.toml`]: https://github.com/MaterializeInc/materialize/tree/main/src/materialized/Cargo.toml
[homebrew-guide]: https://github.com/MaterializeInc/homebrew-materialize/blob/master/CONTRIBUTING.md
[release notes]: https://github.com/MaterializeInc/materialize/tree/main/doc/user/content/release-notes.md
[releases]: https://github.com/MaterializeInc/materialize/releases
[v0.1.2]: https://github.com/MaterializeInc/materialize/releases/tag/v0.1.2
[schedule]: https://docs.google.com/spreadsheets/d/1kd0Tlkr-dKClFLMJuSxhXgcikiZUcKrEFwdc53CBHy0
