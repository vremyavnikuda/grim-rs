const {createHash} = require('node:crypto');
const fs = require('node:fs');
const catalog = require('../labels.json');
const {labelDiff, TYPES, PRIORITIES, STAGES} = require('./label-policy.cjs');
const OLD = 'status:needs-review', NEW = 'status:need-review';
const hash = value => createHash('sha256').update(JSON.stringify(value)).digest('hex');
const repositoryName = repo => `${repo.owner}/${repo.repo}`.toLowerCase();

async function snapshot({github,repo}) {
  const labels = await github.paginate(github.rest.issues.listLabelsForRepo,{...repo,per_page:100});
  const issues = await github.paginate(github.rest.issues.listForRepo,{...repo,state:'open',per_page:100});
  const items=[];
  for (const issue of issues) {
    const assigned=issue.labels;
    if(!Array.isArray(assigned)||assigned.some(label=>!label?.name))throw Error(`Incomplete labels for #${issue.number}`);
    items.push({number:issue.number,pr:!!issue.pull_request,updated_at:issue.updated_at,
      labels:assigned.map(label=>label.name).sort()});
  }
  return {
    labels:labels.map(({name,color,description})=>({name,color,description:description||''})).sort((a,b)=>a.name.localeCompare(b.name)),
    items:items.sort((a,b)=>a.number-b.number),
  };
}

function makePlan(repo,state) {
  const labels = new Map(state.labels.map(label=>[label.name,label]));
  const rename = labels.has(OLD) && !labels.has(NEW);
  const operations=[];
  for (const desired of catalog) {
    const old = labels.get(desired.name);
    if (desired.name===NEW && rename) operations.push({action:'rename',name:OLD,new_name:NEW,color:desired.color,description:desired.description});
    else if (!old) operations.push({action:'create',...desired});
    else if (old.color!==desired.color || old.description!==desired.description) operations.push({action:'update',...desired});
  }
  const warnings=[];
  const items=state.items.map(item=>{
    const current=rename?item.labels.map(label=>label===OLD?NEW:label):item.labels;
    const desired=current.map(label=>label===OLD?NEW:label);
    if (!item.pr && !desired.includes('triage:confirmed')) desired.push('triage:confirmed');
    if (desired.includes('status:blocked') && !desired.some(label=>label.startsWith('blocker:'))) desired.push('blocker:manual');
    for (const [group,names] of [['type',TYPES],['priority',PRIORITIES],['PR stage',STAGES]]) {
      if (group==='PR stage'&&!item.pr) continue;
      if (current.filter(label=>names.includes(label)).length>1) warnings.push(`#${item.number}: conflicting ${group} labels; preserved for maintainer review`);
    }
    if (current.includes('platform:cross')) warnings.push(`#${item.number}: platform:cross requires individual review; preserved`);
    return {number:item.number,...labelDiff(current,desired)};
  }).filter(item=>item.add.length||item.remove.length);
  if (labels.has(OLD)&&labels.has(NEW)) warnings.push('Both review-label names exist; assignments migrate, but the old catalog label requires later manual cleanup');
  return {version:1,repository:repositoryName(repo),snapshotHash:hash(state),catalog:operations,items,warnings};
}

async function prepare(options) { return makePlan(options.repo,await snapshot(options)); }

async function applyPlan({github,repo,plan}) {
  if (plan.version!==1||plan.repository!==repositoryName(repo)) throw Error('Migration plan targets another repository or version');
  const expected=await prepare({github,repo});
  if (expected.snapshotHash!==plan.snapshotHash) throw Error('Repository changed since this plan was prepared; regenerate and review it');
  if (hash(expected.catalog)!==hash(plan.catalog)||hash(expected.items)!==hash(plan.items)) throw Error('Migration operations differ from the reviewed policy');
  for (const operation of plan.catalog) {
    const {action,...parameters}=operation;
    if (action==='create') await github.rest.issues.createLabel({...repo,...parameters});
    else await github.rest.issues.updateLabel({...repo,...parameters});
  }
  for (const item of plan.items) {
    if (item.add.length) await github.rest.issues.addLabels({...repo,issue_number:item.number,labels:item.add});
    for (const name of item.remove) await github.rest.issues.removeLabel({...repo,issue_number:item.number,name});
  }
}

async function main(args) {
  const usage='Usage: node .github/scripts/migrate-labels.cjs --repo OWNER/REPO --report PLAN.json | --apply PLAN.json';
  const options={};
  for(let i=0;i<args.length;i+=2){
    if(!['--repo','--report','--apply'].includes(args[i])||!args[i+1]||options[args[i]])throw Error(usage);
    options[args[i]]=args[i+1];
  }
  if(!/^[\w.-]+\/[\w.-]+$/.test(options['--repo']||'')||!!options['--report']===!!options['--apply'])throw Error(usage);
  const [owner,repo]=options['--repo'].split('/');
  const github=require('./github-cli.cjs').createGithub();
  if(options['--apply']) {
    await applyPlan({github,repo:{owner,repo},plan:JSON.parse(fs.readFileSync(options['--apply'],'utf8'))});
    console.log('Reviewed migration applied. Run labeler reconciliation after deployment.');
  } else {
    const plan=await prepare({github,repo:{owner,repo}});
    fs.writeFileSync(options['--report'],JSON.stringify(plan,null,2)+'\n',{encoding:'utf8',flag:'wx'});
    console.log(`Read-only migration plan saved to ${options['--report']}`);
  }
}

if(require.main===module)main(process.argv.slice(2)).catch(error=>{console.error(error.message);process.exitCode=1;});
module.exports={prepare,applyPlan};
