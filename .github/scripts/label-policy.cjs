const TYPES = ['bug', 'enhancement', 'question'];
const CLASSIFICATION = [...TYPES, 'platform:windows', 'platform:linux', 'platform:cross', 'backend:wayland', 'backend:x11'];
const STAGES = ['status:in-progress', 'status:need-review', 'status:needs-rework', 'status:needs-testing', 'status:ready-to-merge'];
const STATUSES = [...STAGES, 'status:blocked', 'status:needs-review'];
const AUTO_BLOCKERS = ['blocker:ci', 'blocker:merge-conflict'];
const PRIORITIES = ['priority:critical', 'priority:high', 'priority:medium', 'priority:low'];
const RESOLUTIONS = ['resolution:completed', 'resolution:unconfirmed', 'duplicate', 'stale', 'wontfix'];
const KIND = new Map([['Bug report', 'bug'], ['Feature request', 'enhancement'], ['Question', 'question']]);

function parseForm(body) {
  const sections = new Map();
  let section = null;
  let fence = null;
  for (const line of (body || '').replace(/\r/g, '').split('\n')) {
    const marker = line.match(/^ {0,3}(`{3,}|~{3,})(.*)$/);
    if (fence) {
      if (marker && marker[1][0] === fence[0] && marker[1].length >= fence.length && !marker[2].trim()) fence = null;
      continue;
    }
    if (marker) { fence = marker[1]; continue; }
    const heading = line.match(/^### (.+?)\s*$/);
    if (heading) {
      section = heading[1];
      if (sections.has(section)) return null;
      sections.set(section, []);
    } else if (section) sections.get(section).push(line);
  }
  const value = key => sections.get(key)?.join('\n').trim();
  const kind = KIND.get(value('Issue kind'));
  const choices = (key, concrete, exclusive) => {
    const values = value(key)?.split(/,\s*|\n/).map(v => v.trim()).filter(Boolean);
    if (!values?.length || new Set(values).size !== values.length) return null;
    if (values.some(v => ![...concrete, ...exclusive].includes(v))) return null;
    if (values.some(v => exclusive.includes(v)) && values.length !== 1) return null;
    return values;
  };
  const platforms = choices('Affected platforms', ['Windows', 'Linux'], ['Unknown', 'Not platform-specific']);
  const backends = choices('Linux backends', ['Wayland', 'X11'], ['Other', 'Unknown', 'Not applicable']);
  if (!kind || !platforms || !backends) return null;
  if (backends.some(v => ['Wayland', 'X11'].includes(v)) && !platforms.includes('Linux')) return null;
  return [kind, ...['Windows', 'Linux'].filter(v => platforms.includes(v)).map(v => `platform:${v.toLowerCase()}`),
    ...['Wayland', 'X11'].filter(v => backends.includes(v)).map(v => `backend:${v.toLowerCase()}`)];
}

function replaceGroup(labels, group, desired) {
  return [...new Set([...labels.filter(label => !group.includes(label)), ...desired])];
}

function classify(labels, body) {
  if (labels.includes('triage:confirmed')) return labels;
  const parsed = parseForm(body);
  return parsed ? replaceGroup(labels, CLASSIFICATION, parsed) : labels;
}

function ciState(runs, required, wasBlocked) {
  let complete = true;
  let failed = false;
  for (const [path, expected] of Object.entries(required)) {
    const run = runs.find(run => run.path === path && run.current);
    if (!run) { complete = false; continue; }
    if (run.priorFailure) failed = true;
    const conclusions = Object.values(expected).map(name => run.jobs[name]);
    if (conclusions.some(result => result && result !== 'success')) failed = true;
    if (run.status === 'completed' && run.conclusion && run.conclusion !== 'success') failed = true;
    if (run.status !== 'completed' || run.conclusion !== 'success' || conclusions.some(result => result !== 'success')) complete = false;
  }
  return { blocked: failed || (!complete && wasBlocked), success: complete && !failed };
}

function blockers(snapshot) {
  let labels = snapshot.labels;
  if (snapshot.closed) return replaceGroup(labels, AUTO_BLOCKERS, []);
  if (snapshot.conflict !== null && snapshot.conflict !== undefined) {
    labels = replaceGroup(labels, ['blocker:merge-conflict'], snapshot.conflict ? ['blocker:merge-conflict'] : []);
  }
  if (snapshot.ci) labels = replaceGroup(labels, ['blocker:ci'], snapshot.ci.blocked ? ['blocker:ci'] : []);
  return labels;
}

function isBlocked(labels) {
  return [...AUTO_BLOCKERS, 'blocker:manual'].some(label => labels.includes(label));
}

function issueLabels(snapshot) {
  let labels = blockers(snapshot).filter(label => !(snapshot.resetResolutions || []).includes(label));
  if (RESOLUTIONS.includes(snapshot.resolutionChoice) && labels.includes(snapshot.resolutionChoice)) {
    labels = replaceGroup(labels, RESOLUTIONS, [snapshot.resolutionChoice]);
  }
  if (snapshot.closed) {
    if (snapshot.completedByPR && !labels.some(label => RESOLUTIONS.includes(label))) labels.push('resolution:completed');
    return replaceGroup(labels, STATUSES, []);
  }
  if (isBlocked(labels)) return replaceGroup(labels, STATUSES, ['status:blocked']);
  if (snapshot.development === null || snapshot.conflict === null || snapshot.ci === null) return labels;
  if ((snapshot.manualReview === true || snapshot.manualReview === null) && labels.includes('status:need-review')) {
    return replaceGroup(labels, STATUSES, ['status:need-review']);
  }
  return replaceGroup(labels, STATUSES, snapshot.development ? ['status:in-progress'] : []);
}

function prLabels(snapshot, event = {}) {
  let labels = blockers(snapshot);
  if (snapshot.closed) return replaceGroup(labels, STATUSES, []);
  const requested = event.action === 'labeled' && STAGES.includes(event.label) && labels.includes(event.label) ? event.label : null;
  let stage = requested || ['status:needs-rework', 'status:needs-testing', 'status:ready-to-merge', 'status:need-review', 'status:in-progress']
    .find(label => labels.includes(label)) || 'status:in-progress';
  if (snapshot.draft) stage = 'status:in-progress';
  else if (snapshot.resetReview && !['status:needs-rework', 'status:needs-testing'].includes(stage)) stage = snapshot.readyForReview ? 'status:need-review' : 'status:in-progress';
  if (stage === 'status:ready-to-merge' && (isBlocked(labels) || !snapshot.ci?.success || snapshot.conflict !== false)) stage = 'status:need-review';
  labels = replaceGroup(labels, STATUSES, [
    ...(stage === 'status:in-progress' && isBlocked(labels) ? [] : [stage]),
    ...(isBlocked(labels) ? ['status:blocked'] : []),
  ]);
  return labels;
}

function normalizePriority(labels, requested) {
  return PRIORITIES.includes(requested) ? replaceGroup(labels, PRIORITIES, [requested]) : labels;
}

function labelDiff(current, desired) {
  return {
    add: [...new Set(desired)].filter(label => !current.includes(label)).sort(),
    remove: [...new Set(current)].filter(label => !desired.includes(label)).sort(),
  };
}

module.exports = { parseForm, classify, ciState, issueLabels, prLabels, normalizePriority, labelDiff,
  TYPES, CLASSIFICATION, STATUSES, STAGES, AUTO_BLOCKERS, PRIORITIES, RESOLUTIONS };
