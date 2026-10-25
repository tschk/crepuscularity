(function(){
  var idx=[];
  fetch('../docs/docs-search-index.json').then(function(r){return r.json()}).then(function(d){idx=d}).catch(function(){});
  var input=document.getElementById('doc-search-input');
  var results=document.getElementById('doc-search-results');
  var focusIdx=-1;
  if(!input||!results)return;
  function fuzzy(q,s){
    q=q.toLowerCase();s=s.toLowerCase();
    if(s.indexOf(q)!==-1)return 1;
    var qi=0;
    for(var si=0;si<s.length&&qi<q.length;si++){
      if(s[si]===q[qi])qi++;
    }
    return qi===q.length?0.5:0;
  }
  function showAll(){
    var sections=document.querySelectorAll('.nav-section');
    for(var i=0;i<sections.length;i++){
      var lis=sections[i].querySelectorAll('li');
      for(var j=0;j<lis.length;j++)lis[j].style.display='';
    }
  }
  function render(q){
    if(!q.trim()){
      results.className='doc-search-results';
      results.replaceChildren();
      showAll();
      return;
    }
    var scored=[];
    for(var i=0;i<idx.length;i++){
      var s=fuzzy(q,idx[i].title);
      if(s>0)scored.push({item:idx[i],score:s});
    }
    scored.sort(function(a,b){return b.score-a.score});
    /* filter sidebar */
    var matchPaths=scored.map(function(s){return s.item.path});
    var sections=document.querySelectorAll('.nav-section');
    for(var i=0;i<sections.length;i++){
      var lis=sections[i].querySelectorAll('li');
      var anyVisible=false;
      for(var j=0;j<lis.length;j++){
        var a=lis[j].querySelector('a');
        var href=a?a.getAttribute('href'):'';
        var show=matchPaths.indexOf(href)!==-1;
        lis[j].style.display=show?'':'none';
        if(show)anyVisible=true;
      }
      sections[i].open=anyVisible;
    }
    results.replaceChildren();
    if(scored.length===0){
      var empty=document.createElement('div');
      empty.className='no-match';
      empty.textContent='No results';
      results.appendChild(empty);
    }else{
      for(var i=0;i<scored.length;i++){
        var item=scored[i].item;
        var path=String(item&&item.path?item.path:'');
        if(!path||path.indexOf(':')!==-1||path.indexOf('//')!==-1||path.charAt(0)==='/'||path.indexOf('\\')!==-1||path.indexOf('..')!==-1)continue;
        var a=document.createElement('a');
        a.setAttribute('href',path);
        a.textContent=String(item.title||'');
        results.appendChild(a);
      }
    }
    results.className='doc-search-results visible';
    focusIdx=-1;
  }
  input.addEventListener('input',function(){render(this.value)});
  input.addEventListener('keydown',function(e){
    var links=results.querySelectorAll('a');
    if(e.key==='ArrowDown'){e.preventDefault();focusIdx=Math.min(focusIdx+1,links.length-1);updateFocus(links)}
    else if(e.key==='ArrowUp'){e.preventDefault();focusIdx=Math.max(focusIdx-1,-1);updateFocus(links)}
    else if(e.key==='Enter'&&focusIdx>=0&&links[focusIdx]){e.preventDefault();window.location.href=links[focusIdx].href}
    else if(e.key==='Escape'){input.value='';render('');input.blur()}
  });
  function updateFocus(links){
    for(var i=0;i<links.length;i++)links[i].className=i===focusIdx?'focused':'';
  }
  document.addEventListener('click',function(e){
    if(!e.target.closest('.doc-search')){render('')}
  });
})();