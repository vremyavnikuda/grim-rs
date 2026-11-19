# Issue forms and label policy

The policy runs in `vremyavnikuda/grim-rs` through `.github/workflows/labeler.yml`.
It reads GitHub state, reconciles owned labels, and warns/closes inactive issues.
It never executes code from a pull request with its write token.

## Creating an issue

Choose Bug report, Feature request, or Question. Select the affected operating
systems explicitly. Select Linux backends independently; both Wayland and X11
are available. X11 is a context for reports and requests, not a promise of existing
capture support.

Several concrete platforms/backends can be selected. `Unknown`,
`Not platform-specific`, `Other`, and `Not applicable` are exclusive answers in
their respective dropdowns. A concrete Linux backend requires Linux to be
selected. General API questions can be platform-independent.

The issue title and keywords in the description do not assign labels. Malformed
or duplicate form sections preserve existing classification and produce a
workflow warning. API-created issues without the fields need manual triage.

## Classification ownership

| Labels | Owner |
| --- | --- |
| `bug`, `enhancement`, `question` | Form until confirmed; then maintainer |
| `platform:windows`, `platform:linux` | Form until confirmed; then maintainer |
| `backend:wayland`, `backend:x11` | Form until confirmed; then maintainer |
| `triage:confirmed` | Maintainer |
| `documentation`, `performance`, `refactor`, `dependencies` | Maintainer |
| `priority:critical`, `priority:high`, `priority:medium`, `priority:low` | Maintainer; at most one |
| `needs-investigation`, `needs-reproduction`, `waiting-for-feedback` | Maintainer |
| `help wanted`, `good first issue` | Maintainer |
| `stale` | Automation: inactivity warning and closure reason |
| `duplicate`, `invalid`, `wontfix` | Maintainer; do not automatically close issues |
| `resolution:completed` | Maintainer, or verified closure by a merged PR |
| `resolution:unconfirmed` | Maintainer |

To correct classification, change labels and then add `triage:confirmed`.
Subsequent description edits preserve it. Removing confirmation synchronizes
the labels with the current form. Changing another label does not rerun form
classification; the scheduled sweep updates statuses only.

Form edits replace previous automatic selections instead of accumulating them.
Choosing explicit `Unknown`/`Not applicable` clears the corresponding automatic
group. An unreadable form is not treated as an empty selection. Topic and
priority labels are never inferred from free text.

`platform:cross` is a legacy label. Existing assignments require individual
review because the old workflow also used it when no platform was detected.

## Issue working status

An open issue has at most one working status:

| Condition | Status |
| --- | --- |
| At least one blocker | `status:blocked` |
| No blockers, active Development, no manual review request | `status:in-progress` |
| No blockers, maintainer explicitly requested result review | `status:need-review` |
| No blockers, no active Development or manual review request | No working status |

`status:need-review` on an issue is a manual request to check a result, not an
automatic triage state. Automation removes its old bot-assigned instances and
preserves manual assignments. Unknown label history postpones removal.
`triage:confirmed` confirms the report classification and does not start work.

Active Development means an existing linked branch or an open linked PR.
Historical connections, ordinary mentions, and closed/merged PRs alone do not
count. A separately linked branch still counts until it is unlinked or deleted.
Branches without PRs have no PR CI/merge-conflict context to evaluate.

| Blocking cause | Owner |
| --- | --- |
| `blocker:manual` | Maintainer; explain the reason and release condition in a comment |
| `blocker:merge-conflict` | Automation; current conflict in an open linked PR |
| `blocker:ci` | Automation; mandatory PR CI failed and has not fully passed |

Use `blocker:manual`, not a bare `status:blocked`, for a persistent manual block.
CI success cannot remove a manual blocker. Manual blockers belong to the object
on which they are placed; set one on both PR and issue if both must be blocked.
Automatic CI/conflict causes are aggregated across all open linked PRs.

## PR stages and CI

A new open PR starts with `status:in-progress`, whether draft or non-draft.
Its stages are mutually exclusive:

- Implementation: `status:in-progress`.
- Implementation finished, testing requested: maintainer selects `status:needs-testing`,
  replacing `status:in-progress`.
- Review requested: `status:need-review`, selected manually or by ready-for-review.
- Rework or merge approval: maintainer selects `status:needs-rework` or `status:ready-to-merge`.

Blocking replaces `in-progress` with `status:blocked`, but preserves explicit
review/rework/testing stages alongside `status:blocked`. When unblocked, the
selected stage remains; otherwise `in-progress` returns. Blocking removes
`ready-to-merge`, which must be approved again manually after recovery.
New commits/base changes return review/merge readiness to `in-progress`;
explicit rework/testing stages remain. Moving a PR back to draft resumes
`in-progress`. A ready label is
accepted only for a non-draft PR with complete successful mandatory CI, a known
absence of merge conflicts, and no blockers.

All jobs in these workflows are mandatory, defined in `.github/label-policy.json`:

- `CI`: Format, Security audit, Clippy, MSRV (1.82), Checks and Benches on Linux/Windows.
- `Tests`: Linux and Windows.
- `PR Quality`: No merge commits and Conventional commits.

The manifest records workflow paths and job identities, not a magic check count.
Update the manifest together with the workflow definitions when changing CI.

First pending CI does not block. Failure blocks immediately and remains blocking
through retries or new commits until the entire mandatory set passes for the
current PR revision. Terminal cancelled/skipped/neutral jobs do not count as
success. Superseded runs and unrelated workflows do not satisfy the policy.
An API error, unknown mergeability or incomplete result never establishes ready.

The actual target branch ref is read independently: the base SHA embedded in PR
metadata can be stale. The current merge commit's parents must confirm the
head/target pair before absence of conflicts establishes readiness. CI run names
record that pair for cases where GitHub omits fork PR associations. Such a name
is accepted only when the fork workflow file is byte-identical (Git blob SHA) to
the trusted target version and contains the expected run-name expression.
Fork PRs that change those workflow definitions remain unverified when GitHub
omits their associations; they cannot gain ready-to-merge through a forged name.

Labels do not enforce merge permissions. Branch protection/rulesets must be
configured separately if GitHub must prevent merging failed checks.

## Reconciliation and closed objects

### Closure reasons

An issue's working status is separate from its closure reason:

| Reason | Selection |
| --- | --- |
| `resolution:completed` | Explicit maintainer choice, or the merged PR which actually closed the issue |
| `resolution:unconfirmed` | Maintainer could not confirm the report or obtain required information |
| `duplicate` | Maintainer identifies the original issue and includes its link |
| `stale` | Inactivity policy below; does not mean the task was completed |
| `wontfix` | Maintainer decides against implementation or a fix |

Select the reason before or after closing. Selecting a reason does not itself
close the issue. Only one reason is retained; a newer manual choice replaces
the previous reason. Automation never replaces a selected reason with completed.

Automatic completion requires the latest GitHub `ClosedEvent` to identify a
merged PR as the closer, matching the current closure timestamp and completed
state reason. A merely linked/merged PR is insufficient. Ordinary manual Close,
including GitHub's default completed state reason, does not infer this label.
Unknown closure data leaves the reason unchanged.

Reopening removes reasons from the previous closure and recalculates the working
status. A new manual reason selected after reopening remains a new choice.
Type, platform, backend, priority, topic and manual blocker labels are retained.
Scheduled sweeps also repair closure labels and working statuses on closed issues.

### Inactive issues

Every scheduled sweep (every 15 minutes) checks open issues, excluding PRs.
After 30 days without human activity, it posts a warning and adds `stale`.
After seven full days from that warning without activity, it closes the issue
as `not_planned`, retaining `stale`. This never marks the issue as completed.
The first sweep always allows a full seven-day warning period, even for old issues.

Activity includes human comments and edits, report edits, reopening, assignment,
milestone changes and manual label changes. Bot comments, bot edits and automatic
label changes do not reset the timer; the issue's general `updated_at` is not
used as its inactivity clock. Human activity removes `stale` and restarts the
30-day period. Manually adding `stale` alone cannot bypass the warning period.

An active Development link, any `blocker:*`, `priority:high`, `priority:critical`
or `help wanted` exempts an issue and removes `stale`. An explicitly selected
non-stale closure reason is also protected from automatic stale closure. Unknown Development or
incomplete activity data postpones action. Activity and exclusions are checked
again before closure. GitHub cannot atomically check all activity and close an
issue; a reply racing the final close request can require reopening it.

Only warnings authored by `github-actions[bot]` identify an automatic warning
period. Removed stale labels invalidate the previous warning cycle. Dry runs
report proposed actions without comments, labels or closures.

### Working labels

Issue/PR events and trusted CI completion trigger reconciliation. Development
changes are also discovered by the 15-minute scheduled sweep; GitHub schedules
can be delayed. All connections use pagination. API failures preserve unresolved
state and are logged; a later successful sweep repairs it.

All labeler runs share a repository-wide concurrency queue. GitHub's `queue: max`
keeps up to 100 pending runs instead of silently replacing the previous pending
run.

Closing an issue/PR removes working statuses and automatic causes, while keeping
manual labels for history. Reopening recalculates sources. A retained manual
blocker applies again. A reopened PR starts with `in-progress`.

In Actions, run Labeler manually with `dry_run=true` to preview status changes.
Manual runs default to dry-run; event and scheduled runs apply owned differences.

## Migration and deployment

The CLI uses Node.js and an authenticated GitHub CLI (`gh`); no npm packages are
needed. The repository is explicit, so the checkout's old origin URL is irrelevant.

1. Generate a read-only plan outside the repository:

   ```sh
   node .github/scripts/migrate-labels.cjs --repo vremyavnikuda/grim-rs --report /local/path/label-migration.json
   ```

2. Review the JSON catalog changes, per-item changes and warnings. Cross labels
   and conflicting historical classifications are preserved for manual review.
   Existing issue classifications are pinned; unexplained old blocked statuses
   gain a manual reason. The old review-label name is renamed while preserving
   assignments. If both names exist, assignments migrate without deleting either
   catalog entry.

3. Apply only that reviewed plan, before enabling the replacement labeler:

   ```sh
   node .github/scripts/migrate-labels.cjs --repo vremyavnikuda/grim-rs --apply /local/path/label-migration.json
   ```

   A changed snapshot or edited operations rejects the plan before writes. If a
   partial API failure interrupts application, regenerate and review a new plan;
   do not force an old plan or roll back unrelated maintainer changes.

4. Merge the reviewed workflow/code/forms change to the default branch. The
   privileged workflow deliberately checks out that trusted branch, including
   for fork PRs. Then run Labeler with dry-run, inspect the proposed statuses,
   and run reconciliation with `dry_run=false`.

Do not use an issue comment or PR artifact to supply code to the privileged
workflow. Keep migration reports outside the repository.
