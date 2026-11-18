const policy = require('./label-policy.cjs');
const required = require('../label-policy.json');
const RUN_NAME = "${{ github.event_name == 'pull_request' && format('PR #{0} head={1} base={2}', github.event.pull_request.number, github.event.pull_request.head.sha, github.event.pull_request.base.sha) || github.workflow }}";

function createClient({github, context, core, dryRun = false}) {
  const home = context.repo;
  const cache = new Map();
  const warn = message => core.warning(message);
  const labelsFor = (number, repo = home) => github.paginate(github.rest.issues.listLabelsOnIssue,
    {...repo, issue_number:number, per_page:100}).then(labels => labels.map(label => label.name));

  async function connection(number, field, selection) {
    const nodes = [];
    let cursor = null;
    do {
      const result = await github.graphql(`query($owner:String!,$repo:String!,$number:Int!,$cursor:String) {
        repository(owner:$owner,name:$repo) { issue(number:$number) {
          ${field}(first:100,after:$cursor) { nodes { ${selection} } pageInfo { hasNextPage endCursor } }
        } }
      }`, {...home, number, cursor});
      const page = result.repository?.issue?.[field];
      if (!page?.nodes || !page.pageInfo) throw Error(`Incomplete ${field} response for #${number}`);
      nodes.push(...page.nodes);
      const next = page.pageInfo.hasNextPage ? page.pageInfo.endCursor : null;
      if (page.pageInfo.hasNextPage && (!next || next === cursor)) throw Error(`Invalid ${field} pagination`);
      cursor = next;
    } while (cursor);
    return nodes;
  }

  async function readDevelopment(number) {
    const branches = await connection(number, 'linkedBranches', 'ref { name }');
    const prs = await connection(number, 'closedByPullRequestsReferences', 'number state repository { nameWithOwner }');
    if (branches.some(branch => !branch || !Object.hasOwn(branch,'ref') || branch.ref !== null && !branch.ref.name) ||
      prs.some(pr => !pr?.repository?.nameWithOwner || !['OPEN','CLOSED','MERGED'].includes(pr.state))) throw Error('Incomplete Development links');
    const open = prs.filter(pr => pr.state === 'OPEN');
    return {development:branches.some(branch => branch.ref !== null) || open.length > 0, prs:open};
  }

  async function targetHead(pr,repo) {
    const result=await github.rest.git.getRef({...repo,ref:`heads/${pr.base.ref}`});
    if (!result.data?.object?.sha) throw Error('Missing current target branch');
    return result.data.object.sha;
  }

  async function readCI(pr, labels, repo = home, targetSha) {
    targetSha ??= await targetHead(pr,repo);
    const normalized = [];
    let unavailable=false;
    for (const [path] of Object.entries(required)) {
      try {
        const workflow = (await github.rest.actions.getWorkflow({...repo, workflow_id:path.split('/').pop()})).data;
        if (workflow?.path !== path || !workflow.id) throw Error(`Unexpected workflow identity: ${path}`);
        const runs = await github.paginate(github.rest.actions.listWorkflowRuns,
          {...repo, workflow_id:workflow.id, event:'pull_request', head_sha:pr.head.sha, per_page:100});
        const candidates = runs.filter(run => run.event === 'pull_request' && run.head_sha === pr.head.sha &&
          run.workflow_id === workflow.id && run.path === path &&
          run.repository?.full_name?.toLowerCase() === `${repo.owner}/${repo.repo}`.toLowerCase());
        let selected;
        for (const candidate of candidates.sort((a,b) => b.id - a.id)) {
          const associated = candidate.pull_requests || [];
          let current = associated.some(link => link.number === pr.number && link.head?.sha === pr.head.sha &&
            link.base?.sha === targetSha && link.base?.repo?.id === pr.base.repo.id &&
            link.head?.repo?.id === pr.head.repo?.id);
          if (!associated.length && candidate.display_title===`PR #${pr.number} head=${pr.head.sha} base=${targetSha}`) {
            const [owner,name]=(pr.head.repo?.full_name||'').split('/');
            if (!owner||!name) throw Error('Missing workflow source repository');
            const trusted=(await github.rest.repos.getContent({...repo,path,ref:targetSha})).data;
            const source=(await github.rest.repos.getContent({owner,repo:name,path,ref:pr.head.sha})).data;
            // shortcut: changed fork workflow definitions stay unverified; use protected reusable CI to support them.
            const header=trusted.encoding==='base64'?Buffer.from(trusted.content,'base64').toString('utf8').split(/\r?\n/).filter(line=>line.startsWith('run-name: ')):[];
            current=!!trusted.sha && source.sha===trusted.sha && header.length===1 && header[0]===`run-name: "${RUN_NAME}"`;
          }
          if (current) { selected = candidate; break; }
        }
        if (!selected) continue;
        const result={path,current:true,status:selected.status,conclusion:selected.conclusion,jobs:{}};
        normalized.push(result);
        const jobs = await github.paginate(github.rest.actions.listJobsForWorkflowRun,
          {...repo, run_id:selected.id, filter:'all', per_page:100});
        const latest = new Map();
        for (const job of jobs) {
          if (!job.name || !Number.isSafeInteger(job.id)) throw Error('Incomplete workflow job');
          if (!latest.has(job.name) || job.id > latest.get(job.name).id) latest.set(job.name, job);
        }
        result.jobs=Object.fromEntries([...latest].map(([name,job]) => [name, job.status === 'completed' ? job.conclusion : null]));
        result.priorFailure=!(selected.status==='completed'&&selected.conclusion==='success') &&
          jobs.some(job=>Object.values(required[path]).includes(job.name)&&job.status==='completed'&&job.conclusion&&job.conclusion!=='success');
      } catch(error) {unavailable=true;warn(`PR #${pr.number}: ${path} unavailable: ${error.message}`);}
    }
    const result=policy.ciState(normalized, required, labels.includes('blocker:ci'));
    return unavailable&&!result.blocked?null:result;
  }

  async function readPR(number, repo = home) {
    const key = `${repo.owner}/${repo.repo}#${number}`;
    if (cache.has(key)) return cache.get(key);
    const pr = (await github.rest.pulls.get({...repo,pull_number:number})).data;
    const labels = await labelsFor(number, repo);
    let ci = null, targetSha=null;
    let conflict=pr.mergeable===false?true:null;
    if (pr.state === 'open') {
      try {
        targetSha=await targetHead(pr,repo);
        ci = await readCI(pr, labels, repo, targetSha);
        if (pr.mergeable===true && pr.merge_commit_sha) {
          const merge=(await github.rest.git.getCommit({...repo,commit_sha:pr.merge_commit_sha})).data;
          if (merge.parents?.length===2 && merge.parents[0].sha===targetSha && merge.parents[1].sha===pr.head.sha) conflict=false;
        }
      }
      catch (error) { warn(`${key}: cannot establish CI state: ${error.message}`); }
    }
    const snapshot = {pr, labels, closed:pr.state !== 'open', draft:pr.draft, ci,targetSha,conflict};
    cache.set(key, snapshot);
    return snapshot;
  }

  async function stillCurrent(snapshot,repo=home) {
    const fresh=(await github.rest.pulls.get({...repo,pull_number:snapshot.pr.number})).data;
    return fresh.head.sha===snapshot.pr.head.sha && fresh.base.ref===snapshot.pr.base.ref &&
      fresh.state===snapshot.pr.state && fresh.draft===snapshot.pr.draft &&
      (snapshot.closed || snapshot.targetSha===null || await targetHead(fresh,repo)===snapshot.targetSha);
  }

  async function apply(number, current, desired) {
    const diff = policy.labelDiff(current, desired);
    if (!diff.add.length && !diff.remove.length) return;
    core.info(`#${number}: ${JSON.stringify(diff)}${dryRun ? ' (dry run)' : ''}`);
    if (dryRun) return;
    if (diff.add.length) await github.rest.issues.addLabels({...home, issue_number:number, labels:diff.add});
    for (const name of diff.remove) {
      try { await github.rest.issues.removeLabel({...home, issue_number:number, name}); }
      catch (error) { if (error.status !== 404 || !(await labelsFor(number)).every(label => label !== name)) throw error; }
    }
  }

  const eventFor = async (number,current) => {
    const payload = context.payload;
    const target = payload.issue?.number || payload.pull_request?.number;
    if (target!==number) return {};
    const event={action:payload.action,label:payload.label?.name};
    const group=policy.PRIORITIES.includes(event.label)?policy.PRIORITIES:policy.STAGES.includes(event.label)?policy.STAGES:[];
    if (event.action==='labeled' && current.includes(event.label) && current.filter(label=>group.includes(label)).length>1) {
      const timeline=await github.paginate(github.rest.issues.listEventsForTimeline,{...home,issue_number:number,per_page:100});
      const latest=timeline.filter(entry=>entry.event==='labeled'&&group.includes(entry.label?.name)&&current.includes(entry.label.name))
        .sort((a,b)=>b.id-a.id)[0];
      if (!latest) throw Error(`Cannot establish latest manual label choice for #${number}`);
      event.label=latest.label.name;
    }
    return event;
  };

  async function syncIssue(number, classifyForm = false) {
    const issue = (await github.rest.issues.get({...home,issue_number:number})).data;
    let development = null, ci = null, conflict = null;
    if (issue.state === 'open') {
      try {
        const links = await readDevelopment(number);
        development = links.development;
        const states = [];
        for (const link of links.prs) {
          const [owner,repo] = link.repository.nameWithOwner.split('/');
          try { states.push({...await readPR(link.number,{owner,repo}),repo:{owner,repo}}); }
          catch (error) {
            warn(`#${number}: linked PR ${link.repository.nameWithOwner}#${link.number} unavailable: ${error.message}`);
            states.push({ci:null,conflict:null});
          }
        }
        for (const state of states) {
          try {
            if (state.pr && !await stillCurrent(state,state.repo)) {
              state.ci=null;state.conflict=null;development=null;
              warn(`#${number}: linked PR revision changed; preserve unresolved state`);
            }
          } catch(error) {state.ci=null;state.conflict=null;warn(`#${number}: linked PR freshness unknown: ${error.message}`);}
        }
        conflict = states.some(state => state.conflict === true) ? true : states.some(state => state.conflict === null) ? null : false;
        ci = states.some(state => state.ci?.blocked) ? {blocked:true,success:false} :
          states.some(state => !state.ci) ? null : {blocked:false,success:states.every(state => state.ci.success)};
      } catch (error) { warn(`#${number}: cannot establish Development state: ${error.message}`); }
    }
    const fresh = (await github.rest.issues.get({...home,issue_number:number})).data;
    if (fresh.body !== issue.body || fresh.state !== issue.state) {
      warn(`#${number}: issue changed while reading; defer to its next event`); return;
    }
    const current = await labelsFor(number);
    let labels = classifyForm && issue.state === 'open' ? policy.classify(current, issue.body) : current;
    if (classifyForm && !policy.parseForm(issue.body) && !current.includes('triage:confirmed')) warn(`#${number}: missing or invalid form; classification preserved`);
    const event = await eventFor(number,current);
    if (event.action === 'labeled' && current.includes(event.label)) labels = policy.normalizePriority(labels,event.label);
    labels = policy.issueLabels({labels, closed:issue.state !== 'open', development, conflict, ci});
    await apply(number,current,labels);
  }

  async function syncPR(number) {
    const snapshot = await readPR(number);
    if (!await stillCurrent(snapshot)) {
      warn(`#${number}: PR changed while reading; defer to the next reconciliation`); return;
    }
    const current = await labelsFor(number);
    const event = await eventFor(number,current);
    let labels = current;
    if (event.action === 'labeled' && current.includes(event.label)) labels = policy.normalizePriority(labels,event.label);
    const resetReview = ['synchronize','reopened','ready_for_review'].includes(event.action) || !!context.payload.changes?.base;
    await apply(number,current,policy.prLabels({...snapshot,labels,resetReview},event));
  }

  return {readDevelopment, readCI, readPR, syncIssue, syncPR};
}

async function run(options) {
  const {github,context} = options;
  const client = createClient(options);
  if (context.eventName === 'issues') {
    const classify = ['opened','edited'].includes(context.payload.action) ||
      context.payload.action === 'unlabeled' && context.payload.label?.name === 'triage:confirmed';
    await client.syncIssue(context.payload.issue.number,classify);
    return;
  }
  if (context.eventName === 'pull_request_target') await client.syncPR(context.payload.pull_request.number);
  const items = await github.paginate(github.rest.issues.listForRepo,{...context.repo,state:'open',per_page:100});
  if (context.eventName !== 'pull_request_target') {
    for (const item of items.filter(item => item.pull_request)) await client.syncPR(item.number);
  }
  for (const item of items.filter(item => !item.pull_request)) await client.syncIssue(item.number);
}

module.exports = {run,createClient};
