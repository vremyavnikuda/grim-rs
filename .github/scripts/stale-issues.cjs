const DAY = 86400000;
const {RESOLUTIONS} = require('./label-policy.cjs');
const MARKER = '<!-- grim-rs:stale-v1 -->';
const WARNING = `${MARKER}\nThis issue has had no human activity for 30 days and has been marked stale. It will be closed as not planned after 7 more days without activity. Comment, update the report, or resume development to keep it open.`;
const human = actor => actor?.__typename !== 'Bot' && actor?.type !== 'Bot' && !actor?.login?.endsWith('[bot]');
const exempt = labels => labels.some(name => name.startsWith('blocker:') ||
  ['priority:high', 'priority:critical', 'help wanted'].includes(name) || name !== 'stale' && RESOLUTIONS.includes(name));
function time(value) {
  const result = Date.parse(value);
  if (!Number.isFinite(result)) throw Error('Missing or invalid activity timestamp');
  return result;
}

function decide({labels, activity, warningAt, resetAt = 0, development, now}) {
  const stale = labels.includes('stale');
  if (development === null) return 'none';
  if (development || exempt(labels) || now - activity < 30 * DAY || stale && warningAt !== null && activity >= warningAt) return stale ? 'clear' : 'none';
  if (warningAt === null || warningAt <= Math.max(activity, resetAt)) return 'warn';
  if (!stale) return 'mark';
  return now - warningAt >= 7 * DAY ? 'close' : 'none';
}

async function syncIssue({github, repo, core, readDevelopment, issue_number, dryRun = false, allowAging = true, now = Date.now()}) {
  const parameters = {...repo, issue_number};
  async function humanEdit(content) {
    if (!Object.hasOwn(content, 'lastEditedAt')) throw Error('Missing edit metadata');
    if (content.lastEditedAt === null) return 0;
    time(content.lastEditedAt);
    if (!Object.hasOwn(content, 'editor')) throw Error('Missing content editor');
    if (human(content.editor)) return time(content.lastEditedAt);
    if (!content.id) throw Error('Missing content ID for edit history');
    let cursor = null, latest = 0, count = 0;
    do {
      const result = await github.graphql(`query($id:ID!,$cursor:String) {
        node(id:$id) { ... on Comment { userContentEdits(first:100,after:$cursor) {
          nodes { editedAt editor { __typename login } }
          pageInfo { hasNextPage endCursor }
        } } }
      }`, {id:content.id, cursor});
      const page = result.node?.userContentEdits;
      if (!Array.isArray(page?.nodes) || typeof page.pageInfo?.hasNextPage !== 'boolean') throw Error('Incomplete edit history');
      for (const edit of page.nodes) {
        if (!edit) throw Error('Missing content edit');
        count++;
        if (human(edit.editor)) latest = Math.max(latest, time(edit.editedAt));
      }
      const next = page.pageInfo.hasNextPage ? page.pageInfo.endCursor : null;
      if (page.pageInfo.hasNextPage && (!next || next === cursor)) throw Error('Incomplete edit pagination');
      cursor = next;
    } while (cursor);
    if (!count) throw Error('Missing edit history after bot edit');
    return latest;
  }
  async function snapshot() {
    const issue = (await github.rest.issues.get(parameters)).data;
    if (issue.pull_request || issue.state !== 'open') return null;
    if (!Array.isArray(issue.labels) || issue.labels.some(label => !label?.name)) throw Error('Incomplete issue labels');
    const labels = issue.labels.map(label => label.name);
    if (!allowAging && !labels.includes('stale')) return null;
    time(issue.updated_at);
    const state = {issue, labels, activity:time(issue.created_at), warningAt:null, resetAt:0, development:false, now};
    if (exempt(labels) || now - state.activity < 30 * DAY) return state;
    const links = await readDevelopment(issue_number);
    if (typeof links?.development !== 'boolean') throw Error('Unknown Development state');
    state.development = links.development;
    if (state.development) return state;
    const events = await github.paginate(github.rest.issues.listEventsForTimeline, {...parameters, per_page:100});
    if (!Array.isArray(events)) throw Error('Incomplete issue timeline');
    for (const event of events) {
      if (event.event === 'unlabeled' && event.label?.name === 'stale') state.resetAt = Math.max(state.resetAt, time(event.created_at));
      const manual = ['renamed', 'assigned', 'unassigned', 'milestoned', 'demilestoned', 'labeled', 'unlabeled'].includes(event.event) && human(event.actor);
      if (event.event === 'reopened' || manual && !(event.event === 'labeled' && event.label?.name === 'stale')) state.activity = Math.max(state.activity, time(event.created_at));
    }
    let cursor = null;
    do {
      const result = await github.graphql(`query($owner:String!,$repo:String!,$number:Int!,$cursor:String) {
        repository(owner:$owner,name:$repo) { issue(number:$number) {
          id lastEditedAt editor { __typename login }
          comments(first:100,after:$cursor) {
            nodes { id body createdAt lastEditedAt author { __typename login } editor { __typename login } }
            pageInfo { hasNextPage endCursor }
          }
        } }
      }`, {...repo, number:issue_number, cursor});
      const content = result.repository?.issue;
      const page = content?.comments;
      if (!content || !Object.hasOwn(content, 'lastEditedAt') || !Array.isArray(page?.nodes) || typeof page.pageInfo?.hasNextPage !== 'boolean') throw Error('Incomplete issue activity');
      state.activity = Math.max(state.activity, await humanEdit(content));
      for (const comment of page.nodes) {
        if (!comment || typeof comment.body !== 'string' || !Object.hasOwn(comment, 'author')) throw Error('Incomplete issue comment');
        const created = time(comment.createdAt);
        if (human(comment.author)) state.activity = Math.max(state.activity, created);
        state.activity = Math.max(state.activity, await humanEdit(comment));
        if (comment.author?.__typename === 'Bot' && ['github-actions', 'github-actions[bot]'].includes(comment.author.login) &&
            comment.body.startsWith(MARKER + '\n') && (!comment.lastEditedAt || !human(comment.editor))) state.warningAt = Math.max(state.warningAt || 0, created);
      }
      const next = page.pageInfo.hasNextPage ? page.pageInfo.endCursor : null;
      if (page.pageInfo.hasNextPage && (!next || next === cursor)) throw Error('Incomplete comment pagination');
      cursor = next;
    } while (cursor);
    return state;
  }

  let state = await snapshot();
  if (!state) return false;
  let action = decide(state);
  if (action === 'none' || !allowAging && action !== 'clear') return false;
  if (action === 'close') {
    state = await snapshot();
    if (!state) return false;
    action = decide(state);
    if (!['close', 'clear'].includes(action)) return false;
  }
  const fresh = (await github.rest.issues.get(parameters)).data;
  if (fresh.state !== 'open' || fresh.updated_at !== state.issue.updated_at) {
    core.info(`#${issue_number}: stale action deferred after concurrent activity`);
    return false;
  }
  core.info(`#${issue_number}: stale ${action}${dryRun ? ' (dry run)' : ''}`);
  if (dryRun) return false;
  if (action === 'clear') {
    try { await github.rest.issues.removeLabel({...parameters, name:'stale'}); }
    catch (error) { if (error.status !== 404) throw error; }
  } else if (action === 'close') {
    // shortcut: activity checks and closing are not atomic in GitHub; a reply racing this write needs reopening.
    await github.rest.issues.update({...parameters, state:'closed', state_reason:'not_planned'});
    return true;
  } else {
    if (action === 'warn') await github.rest.issues.createComment({...parameters, body:WARNING});
    if (!state.labels.includes('stale')) await github.rest.issues.addLabels({...parameters, labels:['stale']});
  }
  return false;
}

module.exports = {decide, syncIssue, WARNING};
