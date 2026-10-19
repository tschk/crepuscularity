<script lang="ts">
 import {onDestroy} from 'svelte';
 let {id,initial,label}:{id:string;initial:number;label:string}=$props();
 class Base{base=10}
 class Model extends Base{
  // retained public field comment
  before=this.base;
  n:number=$state(initial);
  raw:{count:number}=$state.raw({count:2});
  double:number=$derived(this.n*2);
  plus:number=$derived.by(()=>this.double+1);
  fn:(this:Model)=>number=$state(function(){return this.n});
  tag:(this:Model,strings:TemplateStringsArray)=>number=$state(function(){return this.n});
  after=this.n;
  update(){const previous=this.n++;this.n+=1;({n:this.n}={n:this.n+1});return previous}
 }
 const model=new Model();
 const api=globalThis as unknown as{classModels:Record<string,Model>;classDestroyed:string[]};
 api.classModels??={};api.classDestroyed??=[];api.classModels[id]=model;
 onDestroy(()=>api.classDestroyed.push(id));
</script>
<section><button id={id} onclick={()=>model.n++}>{label}:{model.n}/{model.double}/{model.plus}/{model.fn()}/{model.tag`x`}</button><input value={label}/></section>
