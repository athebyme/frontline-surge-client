'use strict';
// Offline primitive/terrain authoring. Units are meters, Y is up.
  const patterns = ['flat', 'grass', 'dirt', 'wood', 'plank', 'brick', 'plaster', 'stone',
    'roof', 'metal', 'cloth', 'sandbag', 'bark', 'leaves', 'gravel', 'crop', 'glass', 'light',
    'land-cover', 'worn-plaster', 'interior-plaster', 'corrugated', 'slate-camo', 'dune-camo',
    'shutter', 'fissure-a', 'asphalt', 'corrugated-paint', 'concrete', 'fissure-b'];
  const groundColors = {
    meadow: ['#84a967','grass'], grass: ['#769e61','grass'], earth: ['#9e805f','dirt'],
    dirt: ['#bb9769','dirt'], orchard: ['#789b55','grass'], crop: ['#d9b45f','crop'],
    gravel: ['#acaa9a','gravel'], road: ['#c5a276','dirt'], verge: ['#859d69','grass'],
    rut: ['#a08a6b','gravel'], stone: ['#8c8775','stone'], bark: ['#615744','bark'],
    wood: ['#a87b52','wood'], plaster: ['#b6aa91','plaster'], leaves: ['#5e875a','leaves'],
    metal: ['#50554f','metal'], brick: ['#b67559','brick'], roof: ['#92735e','roof'],
    cloth: ['#777456','cloth'], sandbag: ['#a69a73','sandbag'], glass: ['#869697','glass'],
    dryGrass: ['#bba568','grass'], darkGrass: ['#65885b','grass'],
    stubble: ['#baa15d','grass'], wildGrass: ['#719454','grass'],
    distantTree: ['#58796a','leaves'], distantTrunk: ['#6e6853','bark']
  };
  function hash(x,y,k=0) {
    let h = Math.imul(x+17,374761393) + Math.imul(y+53,668265263) + Math.imul(k+91,1274126177) | 0;
    h = Math.imul(h^(h>>>13),1274126177);
    return ((h^(h>>>16))>>>0)/4294967296;
  }
  function rgb(hex) {
    hex=String(hex).replace('#','');if(hex.length===3)hex=hex.split('').map(v=>v+v).join('');
    if(!/^[0-9a-f]{6}$/i.test(hex))hex='9a9079';
    const v=parseInt(hex,16);return [(v>>16)/255,((v>>8)&255)/255,(v&255)/255];
  }
  function unit(v){const l=Math.hypot(...v)||1;return v.map(x=>x/l);}
  function transform(x=0,y=0,z=0,yaw=0,pitch=0,roll=0) {
    const c=Math.cos(yaw),s=Math.sin(yaw),a=Math.cos(pitch),b=Math.sin(pitch),d=Math.cos(roll),e=Math.sin(roll);
    // R_y(-yaw) R_x(pitch) R_z(roll), columns followed by translation.
    return [c*d-s*b*e,a*e,s*d+c*b*e, -c*e-s*b*d,a*d,-s*e+c*b*d, -s*a,-b,c*a,x,y,z];
  }
  function combine(a,b) {
    const m=new Array(12);
    for(let c=0;c<4;c++)for(let r=0;r<3;r++)m[c*3+r]=a[r]*b[c*3]+a[r+3]*b[c*3+1]+a[r+6]*b[c*3+2]+(c===3?a[9+r]:0);
    return m;
  }
  function geometry(type) {
    const data=[];
    function vertex(p,n){data.push(...p,...n);}
    function tri(p,q,r,n,m=n,l=n){vertex(p,n);vertex(q,m);vertex(r,l);}
    function quad(a,b,c,d,n){tri(a,b,c,n);tri(a,c,d,n);}
    const h=.5;
    if(type==='box' || type==='plane'){
      quad([-h,h,-h],[-h,h,h],[h,h,h],[h,h,-h],[0,1,0]);
      if(type==='box'){
        quad([-h,-h,h],[-h,-h,-h],[h,-h,-h],[h,-h,h],[0,-1,0]);
        quad([-h,-h,h],[h,-h,h],[h,h,h],[-h,h,h],[0,0,1]);
        quad([h,-h,-h],[-h,-h,-h],[-h,h,-h],[h,h,-h],[0,0,-1]);
        quad([h,-h,h],[h,-h,-h],[h,h,-h],[h,h,h],[1,0,0]);
        quad([-h,-h,-h],[-h,-h,h],[-h,h,h],[-h,h,-h],[-1,0,0]);
      }
    }else if(type==='gable'){
      tri([-h,-h,h],[h,-h,h],[0,h,h],[0,0,1]);
      tri([h,-h,-h],[-h,-h,-h],[0,h,-h],[0,0,-1]);
      quad([-h,-h,h],[0,h,h],[0,h,-h],[-h,-h,-h],unit([-1,.5,0]));
      quad([h,-h,-h],[0,h,-h],[0,h,h],[h,-h,h],unit([1,.5,0]));
      quad([-h,-h,h],[-h,-h,-h],[h,-h,-h],[h,-h,h],[0,-1,0]);
    }else if(type==='tuft'){
      for(let i=0;i<5;i++){
        const a=i*2.4,b=i%2?.16:-.08;
        const x=.27*Math.cos(a),z=.27*Math.sin(a);
        const dx=.09*Math.cos(a+.8),dz=.09*Math.sin(a+.8);
        tri([x-dx,-.5,z-dz],[x+dx,-.5,z+dz],[x+b,.28+.05*i,z+.11],[0,.8,.6]);
      }
    }else if(type==='crown'||type==='crownFar'){
      // Four staggered rings form broad, uneven planes. Flat face normals make
      // silhouettes readable even when the small leaf marks have faded with distance.
      const sides=type==='crown'?7:5;
      const rings=[[-.49,.15],[-.27,.87],[.10,1.0],[.37,.63],[.49,.13]];
      function point(j,i){
        const k=(i+sides)%sides, ring=rings[j];
        const angle=k*2*Math.PI/sides+(j%2)*.29;
        const radial=.5*ring[1]*(.91+.12*Math.sin(k*2.71+j*.86));
        return [Math.cos(angle)*radial,ring[0]+.022*Math.cos(k*2.3+j),Math.sin(angle)*radial];
      }
      function facet(a,b,c){
        const u=b.map((n,k)=>n-a[k]),v=c.map((n,k)=>n-a[k]);
        let normal=unit([u[1]*v[2]-u[2]*v[1],u[2]*v[0]-u[0]*v[2],u[0]*v[1]-u[1]*v[0]]);
        if(normal[0]*a[0]+normal[1]*a[1]+normal[2]*a[2]<0)normal=normal.map(n=>-n);
        tri(a,b,c,normal);
      }
      for(let i=0;i<sides;i++){
        facet([0,-.50,0],point(0,i+1),point(0,i));
        for(let j=0;j<rings.length-1;j++){
          const a=point(j,i),b=point(j+1,i),c=point(j+1,i+1),d=point(j,i+1);
          facet(a,b,c);facet(a,c,d);
        }
        facet([0,.50,0],point(rings.length-1,i),point(rings.length-1,i+1));
      }
    }else if(type==='sphere'||type==='lowSphere'){
      const rings=type==='sphere'?6:4, sides=type==='sphere'?10:7;
      function point(j,i){const a=j*Math.PI/rings-Math.PI/2,b=i*2*Math.PI/sides;return [Math.cos(a)*Math.cos(b),Math.sin(a),Math.cos(a)*Math.sin(b)];}
      for(let j=0;j<rings;j++)for(let i=0;i<sides;i++){
        const pts=[point(j,i),point(j+1,i),point(j+1,i+1),point(j,i+1)];
        for(const k of (j===0?[0,1,2]:j===rings-1?[0,2,3]:[0,1,2,0,2,3]))vertex(pts[k].map(v=>v*.5),pts[k]);
      }
    }else{
      const sides=Number(type.slice(1));
      for(let i=0;i<sides;i++){
        const a=i*Math.PI*2/sides,b=(i+1)*Math.PI*2/sides,m=(a+b)/2;
        const x=h*Math.cos(a),z=h*Math.sin(a),u=h*Math.cos(b),v=h*Math.sin(b);
        quad([x,-h,z],[x,h,z],[u,h,v],[u,-h,v],[Math.cos(m),0,Math.sin(m)]);
        tri([0,h,0],[u,h,v],[x,h,z],[0,1,0]);
        tri([0,-h,0],[x,-h,z],[u,-h,v],[0,-1,0]);
      }
    }
    return new Float32Array(data);
  }
const STRIDE=18;
  class Batch {
    constructor(){this.data=new Float32Array(STRIDE*8);this.used=0;}
    reserve(n){
      if(this.used+n<=this.data.length)return;
      const next=new Float32Array(Math.max(this.used+n,this.data.length*2));
      next.set(this.data.subarray(0,this.used));this.data=next;
    }
    append(a){this.reserve(a.length);this.data.set(a,this.used);this.used+=a.length;}
  }
class Collector {
    constructor(){
      this._groups=[]; this._pal=new Map(); this._collector=new Map();
    }
    static(_key,build){build();}
    group(x,y,z,yaw,fn,pitch=0,roll=0) {
      const m=transform(x,y,z,yaw,pitch,roll),list=this._groups;
      list.push(list.length?combine(list[list.length-1],m):m);
      try{fn();}finally{list.pop();}
    }
    _batch(type){
      if(!this._collector.has(type))this._collector.set(type,new Batch());
      return this._collector.get(type);
    }
    _material(key){
      if(!this._pal.has(key))this._pal.set(key,[...rgb(key),0]);
      return this._pal.get(key);
    }
    _add(type,x,y,z,w,h,d,mat,yaw,pitch,roll,top=1,bottom=1){
      let m=transform(x,y,z,yaw,pitch,roll);
      const g=this._groups;if(g.length)m=combine(g[g.length-1],m);
      const batch=this._batch(type),p=this._material(mat);
      batch.reserve(STRIDE);const arr=batch.data;let i=batch.used;
      for(let j=0;j<3;j++){
        const s=j===0?w:j===1?h:d;
        arr[i++]=m[j*3]*s;arr[i++]=m[j*3+1]*s;arr[i++]=m[j*3+2]*s;arr[i++]=m[9+j];
      }
      for(let j=0;j<4;j++)arr[i++]=p[j];
      arr[i++]=top;arr[i++]=bottom;batch.used=i;
    }
    box(x,y,z,w,h,d,mat,yaw=0,pitch=0,roll=0){this._add('box',x,y,z,w,h,d,mat,yaw,pitch,roll);}
    gable(x,y,z,w,h,d,mat,yaw=0){this._add('gable',x,y,z,w,h,d,mat,yaw,0,0);}
    crown(x,y,z,w,h,d,mat,yaw=0,pitch=0,roll=0){this._add('crown',x,y,z,w,h,d,mat,yaw,pitch,roll);}
    sphere(x,y,z,w,h,d,mat,yaw=0,pitch=0,roll=0){this._add('sphere',x,y,z,w,h,d,mat,yaw,pitch,roll);}
    cylinder(x,y,z,bottomRadius,topRadius,h,mat,sides=8,yaw=0,pitch=0,roll=0){
      const r=Math.max(.00001,Math.abs(bottomRadius),Math.abs(topRadius));
      const n=sides<=3?3:sides<=4?4:sides<=5?5:sides<=6?6:sides<=8?8:sides<=12?12:16;
      this._add('c'+n,x,y,z,r*2,h,r*2,mat,yaw,pitch,roll,topRadius/r,bottomRadius/r);
    }
    beam(x1,y1,z1,x2,y2,z2,w,d,mat){
      const dx=x2-x1,dy=y2-y1,dz=z2-z1,len=Math.hypot(dx,dy,dz);
      if(len>.0001)this.box((x1+x2)/2,(y1+y2)/2,(z1+z2)/2,w,len,d,mat,Math.atan2(-dx,dz),Math.atan2(Math.hypot(dx,dz),dy));
    }
    _patch(x,y,z,w,d,mat,yaw){this._add('plane',x,y-.0005,z,w,.001,d,mat,yaw,0,0);}
}
module.exports = { geometry, patterns, groundColors, rgb, hash, Collector };
