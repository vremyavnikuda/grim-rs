const {execFileSync}=require('node:child_process');

// Local tools reuse gh authentication; the workflow uses github-script's existing client.
function createGithub(invoke=(args,options)=>execFileSync('gh',args,options)) {
  function call(args,options,readOnly) {
    for(let attempt=0;;attempt++) {
      try {return invoke(args,{...options,timeout:30000});}
      catch(error) {
        const message=error.message+' '+(error.stderr||'');
        if(!readOnly||attempt===2||!/EOF|ETIMEDOUT|ECONNRESET|TLS handshake timeout|HTTP 50[234]/i.test(message))throw error;
      }
    }
  }
  function request(method,template,parameters,paginate=false) {
    const values={...parameters};
    let endpoint=template.replace(/\{(\w+)\}/g,(_,name)=>{
      if(values[name]===undefined)throw Error(`Missing REST path parameter: ${name}`);
      const value=encodeURIComponent(values[name]);delete values[name];return value;
    });
    const args=['api','--hostname','github.com','--method',method];
    let input;
    if(method==='GET') {
      const query=new URLSearchParams(values).toString();if(query)endpoint+='?'+query;
    } else if(Object.keys(values).length) {args.push('--input','-');input=JSON.stringify(values);}
    if(paginate)args.push('--paginate','--slurp');
    args.push(endpoint);
    const output=call(args,{encoding:'utf8',shell:false,input,maxBuffer:32*1024*1024},method==='GET');
    return output.trim()?JSON.parse(output):null;
  }
  function route(method,path) {
    const fn=async parameters=>({data:request(method,path,parameters)});
    fn.paginate=async parameters=>{
      const pages=request(method,path,parameters,true);
      if(!Array.isArray(pages))throw Error('Invalid paginated REST response');
      return pages.flatMap(page=>{
        const items=Array.isArray(page)?page:page?.workflow_runs??page?.jobs??page?.check_suites;
        if(!Array.isArray(items))throw Error('Incomplete paginated REST response');
        return items;
      });
    };
    return fn;
  }
  const base='repos/{owner}/{repo}';
  return {
    rest:{
      issues:{
        get:route('GET',`${base}/issues/{issue_number}`),
        update:route('PATCH',`${base}/issues/{issue_number}`),
        createComment:route('POST',`${base}/issues/{issue_number}/comments`),
        listForRepo:route('GET',`${base}/issues`),
        listLabelsForRepo:route('GET',`${base}/labels`),
        listLabelsOnIssue:route('GET',`${base}/issues/{issue_number}/labels`),
        listEventsForTimeline:route('GET',`${base}/issues/{issue_number}/timeline`),
        addLabels:route('POST',`${base}/issues/{issue_number}/labels`),
        removeLabel:route('DELETE',`${base}/issues/{issue_number}/labels/{name}`),
        createLabel:route('POST',`${base}/labels`),
        updateLabel:route('PATCH',`${base}/labels/{name}`),
      },
      pulls:{get:route('GET',`${base}/pulls/{pull_number}`)},
      repos:{getContent:route('GET',`${base}/contents/{path}`)},
      git:{getRef:route('GET',`${base}/git/ref/{ref}`),getCommit:route('GET',`${base}/git/commits/{commit_sha}`)},
      actions:{
        getWorkflow:route('GET',`${base}/actions/workflows/{workflow_id}`),
        listWorkflowRuns:route('GET',`${base}/actions/workflows/{workflow_id}/runs`),
        listJobsForWorkflowRun:route('GET',`${base}/actions/runs/{run_id}/jobs`),
      },
      checks:{listSuitesForRef:route('GET',`${base}/commits/{ref}/check-suites`)},
    },
    paginate:(method,parameters)=>method.paginate(parameters),
    graphql:async(query,variables)=>{
      const result=JSON.parse(call(['api','--hostname','github.com','graphql','--input','-'],
        {encoding:'utf8',shell:false,input:JSON.stringify({query,variables}),maxBuffer:32*1024*1024},true));
      if(result.errors?.length||!result.data)throw Error('Incomplete or rejected GraphQL response');
      return result.data;
    },
  };
}

module.exports={createGithub};
