'use strict';
// Original buildings, vegetation, vehicles and furniture for the native mesh baker.
  const materials = [
    ['concrete','#bdc1b6','concrete'], ['lime','#e1d7b8','plaster'], ['ochre','#ccae83','plaster'], ['whitewash','#e7d8b8','plaster'],
    ['brickwork','#b76b53','brick'], ['rubble','#827e6b','stone'], ['limestone','#aaa184','stone'],
    ['basalt','#697066','stone'], ['slate','#506777','roof'], ['terracotta','#b8684c','roof'],
    ['tileEdge','#80513e','plaster'], ['lead','#606661','metal'], ['soot','#333a35','plaster'],
    ['quietGlass','#425d66','glass'], ['sunGlass','#82a7aa','glass'], ['void','#292d26','flat'],
    ['joinery','#927252','wood'], ['creamPaint','#c5bba0','wood'], ['shutters','#588075','plank'],
    ['barnBoard','#986545','plank'], ['ciderBoard','#a76c4f','plank'], ['darkWood','#544c39','wood'],
    ['weatherWood','#a18a68','wood'], ['woodcut','#bf9e70','wood'], ['trunk','#786348','bark'],
    ['timber','#82694b','bark'], ['needles','#31594c','leaves'], ['spruce','#42745a','leaves'],
    ['spruceSun','#679065','leaves'], ['leafShadow','#3e6954','leaves'], ['leaf','#60924f','leaves'],
    ['leafSun','#93aa64','leaves'], ['hedge','#517c51','leaves'], ['leafDry','#b2a05f','leaves'],
    ['khakiTarp','#ae986c','cloth'],['tarpSeam','#867b63','cloth'],
    ['oilBlack','#373c37','metal'],['pumpRed','#9d614e','metal'],['creamPlate','#d7c799','metal'],
    ['steel','#55636b','metal'], ['oxide','#a76847','metal'], ['iron','#414846','metal'],
    ['paleSteel','#92978a','metal'], ['tire','#343a35','cloth'], ['truckPaint','#65715a','metal'],
    ['truckDark','#515b49','metal'], ['armor','#788065','metal'], ['gunTube','#737a6a','metal'],
    ['canvas','#97947b','cloth'], ['fieldPaint','#687864','wood'],
    ['sack','#a39670','sandbag'], ['sackShade','#8f8464','sandbag'], ['sackLight','#b2a680','sandbag'],
    ['stencil','#d4c9a0','plaster'], ['mustard','#c29b54','metal'], ['redLead','#995b48','metal'],
    ['aerial','#666b61','metal'], ['flagRed','#c26a4c','cloth'], ['flagBlue','#5d95a1','cloth'],
    ['lamp','#f4cd70','light'], ['buildGo','#76b3a0','light'], ['buildStop','#df8065','light'],
    ['friendMark','#70b5d7','light'],['enemyMark','#db775a','light'],
    ['uniform','#696e61','cloth'], ['uniformDark','#4a5349','cloth'], ['wool','#767a6b','cloth'],
    ['belt','#584732','wood'], ['hide','#68523d','cloth'], ['skin','#c59b76','cloth'],
    ['helmet','#505b50','metal'], ['strap','#90805f','cloth'], ['glove','#74664e','cloth'],
    ['sleeve','#707867','cloth'], ['sleeveShade','#555f50','cloth'], ['stock','#8b5a3c','wood'],
    ['stockLight','#a4734d','wood'], ['bluing','#474c47','metal'], ['receiver','#555b55','metal'],
    ['barrelEdge','#72796d','metal'], ['brass','#b09457','metal'],
    ['flash','#ffdd84','light'], ['flashCore','#fff4ba','light'], ['redTracer','#ffc58e','light'],
    ['smoke','#888a7a','cloth']
  ];

  const TAU = Math.PI * 2;
  const halfPi = Math.PI / 2;
  const clamp = (v,low,high) => Math.min(high,Math.max(low,v));
  function rng(seed) {
    let n = seed >>> 0;
    return function() {
      n = (Math.imul(n,1664525) + 1013904223) >>> 0;
      return n / 4294967296;
    };
  }
  function seedOf(p) {
    return Math.imul(Math.round(p.x*101),73856093) ^ Math.imul(Math.round(p.z*117),19349663);
  }
  function stick(r,x1,y1,z1,x2,y2,z2,width,depth,mat) {
    r.beam(x1,y1,z1,x2,y2,z2,width,depth,mat);
  }
  function taper(r,from,to,base,tip,mat,sides=8) {
    const dx=to[0]-from[0],dy=to[1]-from[1],dz=to[2]-from[2];
    r.cylinder((from[0]+to[0])*.5,(from[1]+to[1])*.5,(from[2]+to[2])*.5,
      base,tip,Math.hypot(dx,dy,dz),mat,sides,Math.atan2(-dx,dz),Math.atan2(Math.hypot(dx,dz),dy));
  }
  function facadeWindow(r,x,y,z,size,height,paint,shutters=true) {
    const s = Math.sign(z);
    r.box(x,y,z,size+.16,height+.19,.13,'limestone');
    r.box(x,y,z+s*.085,size,height,.09,'void');
    for (const side of [-1,1]) {
      r.box(x+side*size*.225,y+.015,z+s*.138,size*.385,height*.83,.035,side<0?'quietGlass':'sunGlass');
      if (shutters) {
        const sx=x+side*(size*.72+.08);
        r.box(sx,y,z+s*.081,size*.40,height+.04,.095,paint);
        for (const bar of [-.31,.31]) r.box(sx,y+height*bar,z+s*.14,size*.4,.055,.037,'darkWood');
      }
    }
    r.box(x,y,z+s*.163,.052,height,.048,'creamPaint');
    r.box(x,y+height*.11,z+s*.165,size,.052,.049,'creamPaint');
    r.box(x,y-height*.52,z+s*.11,size+.28,.10,.27,'limestone');
  }
  function sideWindow(r,side,x,y,z,size,height) {
    r.group(x,y,z,side>0?-halfPi:halfPi,()=>{
      facadeWindow(r,0,0,.02,size,height,'shutters',false);
    });
  }
  function doorway(r,x,z,width,height) {
    const s = Math.sign(z);
    r.box(x,height*.5+.09,z,width+.19,height+.23,.16,'limestone');
    r.box(x,height*.5+.06,z+.10*s,width,height,.09,'darkWood');
    r.box(x,height*.5+.065,z+.152*s,width*.87,height*.94,.038,'shutters');
    r.box(x,height*.65,z+.181*s,width*.60,height*.33,.032,'darkWood');
    r.box(x,height*.65,z+.207*s,width*.47,height*.25,.026,'quietGlass');
    r.box(x+width*.31,height*.46,z+.217*s,width*.13,.056,.067,'mustard');
    r.box(x,.095,z+.18*s,width+.35,.19,.65,'limestone');
  }
  function pitchedRoof(r,w,d,h,eave,roofColor,trimColor) {
    const run=w*.5+.27;
    const rise=(h-eave)*(run/(w*.5));
    const len=Math.hypot(run,rise), slope=Math.atan2(rise,run);
    for(const side of [-1,1]) {
      r.box(side*run*.5,h-rise*.5+.025,0,len,.16,d+.62,roofColor,0,0,-side*slope);
      for (const end of [-1,1]) {
        r.box(side*run*.5,h-rise*.5-.064,end*(d*.5+.25),len,.14,.115,trimColor,0,0,-side*slope);
      }
      r.box(side*run,h-rise-.038,0,.13,.15,d+.64,trimColor);
    }
    r.box(0,h+.09,0,.17,.15,d+.64,trimColor);
  }
  function home(r,p,random) {
    const w=p.w||7,d=p.d||7,h=p.h||6;
    const v=p.variant??(seedOf(p)>>>0)%4;
    const barn=p.kind==='barn';
    const eave=h*(barn?.64:.67);
    const tall=!barn&&eave>=4.7;
    const mat=barn?(v%2?'ciderBoard':'barnBoard'):['lime','whitewash','ochre','lime'][v%4];
    r.box(0,.23,0,w+.10,.46,d+.1,'rubble');
    r.box(0,eave*.5+.15,0,w,eave-.30,d,mat);
    r.gable(0,(h+eave)*.5,0,w,h-eave,d,mat);
    pitchedRoof(r,w,d,h,eave,(v===1&&!barn)||v===3?'terracotta':'slate',barn?'darkWood':v%2?'tileEdge':'lead');
    for(const side of [-1,1]){
      r.box(side*(w*.5-.07),eave*.5,0,.14,.18,d+.05, barn?'barnBoard':mat);
      if(barn) {
        r.box(side*(w*.5+.035),eave-.11,0,.13,.23,d+.05,'darkWood');
        for(let k=-1;k<=1;k++) r.box(side*(w*.5+.055),eave*.50,k*d*.39,.17,eave,.16,'darkWood');
      } else {
        for (const loc of [-.27,.27]) {
          sideWindow(r,side,side*(w*.5+.04),tall?1.42:eave*.54,loc*d,Math.min(1.02,d*.15),Math.min(1.35,eave*.38));
          if(tall)sideWindow(r,side,side*(w*.5+.04),eave-.96,loc*d,Math.min(.98,d*.14),1.22);
        }
      }
    }
    if(!barn) {
      const z=d*.5+.02, win=Math.min(1.2,w*.16), wh=Math.min(1.45,eave*.40);
      const doorX=w*.17;
      doorway(r,doorX,z,Math.min(1.16,w*.19),Math.min(2.25,eave*.73));
      facadeWindow(r,-w*.26,tall?1.50:eave*.56,z,win,wh,'shutters');
      for (const pos of [-.25,.25]) {
        facadeWindow(r,w*pos,tall?1.44:eave*.54,-z,win,wh,'shutters',false);
        if(tall){
          facadeWindow(r,w*pos,eave-1.02,z,win*.91,1.25,'shutters');
          facadeWindow(r,w*pos,eave-1.02,-z,win*.91,1.25,'shutters',false);
        }
      }
      if(tall)r.box(0,2.85,z-.029,w+.07,.075,.085,'limestone');
      facadeWindow(r,0,eave+(h-eave)*.33,z,Math.min(.67,w*.09),Math.min(.7,(h-eave)*.34),'shutters',false);
      for(const sx of [-1,1])for(const sz of [-1,1]) {
        for(let i=0;i<Math.min(6,Math.floor(eave/.47));i++)
          r.box(sx*(w*.5-.09),.54+i*.50,sz*(d*.5-.07),i%2?.20:.32,.23,i%2?.32:.21,'limestone');
      }
      const cx=w*.22,cz=-d*.19,ch=h+.63;
      r.box(cx,(eave+ch)*.5,cz,.57,ch-eave,.62,'brickwork');
      r.box(cx,ch,cz,.68,.17,.72,'rubble');
      r.cylinder(cx,ch+.22,cz,.15,.13,.32,'tileEdge',8);
      r.cylinder(cx,ch+.39,cz,.113,.113,.025,'void',8);
      if(v%2===1) {
        r.box(doorX,Math.min(2.55,eave*.85),z+.28,1.62,.12,.73,'slate',0,.12);
        for(const sx of [-1,1]) r.box(doorX+sx*.65,Math.min(2.31,eave*.77),z+.15,.08,.38,.08,'darkWood',0,.4);
      }
    } else {
      const z=d*.5+.04,bw=Math.min(w*.64,4.4),bh=Math.min(eave*.89,3.45);
      r.box(0,bh*.5+.01,z,bw+.25,bh+.2,.12,'darkWood');
      for(const s of [-1,1]) {
        const cx=s*bw*.25;
        r.box(cx,bh*.50+.04,z+.085,bw*.485,bh*.98,.10,'barnBoard');
        for(const y of [.13,.85]) r.box(cx,bh*y,z+.16,bw*.46,.12,.08,'weatherWood');
        stick(r,cx-bw*.21,bh*.15,z+.17,cx+bw*.21,bh*.84,z+.17,.115,.10,'weatherWood');
      }
      r.box(0,eave+(h-eave)*.33,z,Math.min(1.1,w*.17),Math.min(.75,(h-eave)*.42),.14,'void');
      for(const s of [-1,1])r.box(s*(w*.5-.12),eave*.50,z,.20,eave,.17,'darkWood');
      r.box(0,eave-.13,z,w-.15,.24,.19,'darkWood');
    }
    // Small patches of exposed masonry are flush with the front plaster.
    if(!barn && v!==1) for(let i=0;i<6;i++){
      const x=-w*.45+random()*w*.16,y=.48+random()*.55;
      r.box(x,y,d*.5+.012,.17+random()*.24,.07+random()*.09,.035,'rubble');
    }
  }
  function ruin(r,p,random) {
    const w=p.w||6,d=p.d||5,h=p.h||4;
    r.box(0,.15,0,w+.22,.30,d+.23,'rubble');
    const blocks=7,step=w/blocks;
    for(let i=0;i<blocks;i++){
      const x=-w*.5+(i+.5)*step, shape=[.86,.94,.78,.50,.59,.41,.28][i];
      const hh=h*shape;
      r.box(x,hh*.5,-d*.5,step+.016,hh,.38,'rubble');
      r.box(x,hh+.075,-d*.5,step*.81,.15,.41,'brickwork');
      if(i<2 || i>4){
        const fh=h*(i<2?.63-i*.15:.25+random()*.10);
        r.box(x,fh*.5,d*.5,step+.008,fh,.34,'rubble');
        r.box(x,fh+.065,d*.5,step*.70,.13,.37,'limestone');
      }
    }
    for(const s of [-1,1])for(let i=0;i<5;i++){
      const z=-d*.40+i*d*.20,hh=h*(s<0?.75-i*.104:.30+random()*.12);
      r.box(s*(w*.5-.16),hh*.5,z,.32,hh,d*.204,'rubble');
    }
    for(let i=0;i<16;i++){
      const x=(random()-.5)*w*.88,z=(random()-.5)*d*.92;
      r.box(x,.19+random()*.1,z,.25+random()*.55,.19+random()*.25,.25+random()*.45,
            i%3?'rubble':'brickwork',random()*TAU,random()*.22,random()*.35);
    }
    stick(r,-w*.43,h*.65,-d*.43,w*.16,.36,d*.36,.18,.2,'soot');
    stick(r,-w*.43,h*.49,-d*.20,-w*.08,.25,d*.39,.15,.17,'darkWood');
  }
  function foliage(r,p,random) {
    const h=p.h||7,w=p.w||h*.58,d=p.d||w;
    if(p.kind==='bush'){
      for(let i=0;i<5;i++){
        const a=i*2.39996, radius=.12+.17*random();
        r.crown(Math.cos(a)*w*radius,h*(.34+random()*.18),Math.sin(a)*d*radius,
          w*(.52+random()*.23),h*(.63+random()*.33),d*(.52+random()*.19),
          ['leafShadow','hedge','leaf','leafSun'][i%4],a);
      }
      return;
    }
    r.cylinder(0,h*.29,0,w*.083,w*.041,h*.58,'trunk',7);
    if(p.kind==='pine'){
      for(let tier=0;tier<4;tier++){
        const level=.19+tier*.178, radius=w*(.51-tier*.105), span=h*(.45-tier*.053);
        r.cylinder(0,h*level+span*.5,0,radius,radius*.04,span,['needles','spruce','spruce','spruceSun'][tier],7,tier*.47);
        for(let j=0;j<3;j++){
          const a=j*TAU/3+tier*.61,reach=radius*.45;
          r.cylinder(Math.cos(a)*reach,h*level+span*.27,Math.sin(a)*reach,
            radius*.46,.02,span*.58,j%3?'spruce':'needles',5,a);
        }
      }
      return;
    }
    const spread=1+random()*.18, oak=(p.variant||0)%2===0;
    for(let i=0;i<5;i++){
      const a=i*TAU/5+random()*.26, reach=.20+random()*.095;
      const ex=Math.cos(a)*w*reach*spread,ez=Math.sin(a)*d*reach,ey=h*(.57+random()*.09);
      taper(r,[0,h*(.23+random()*.11),0],[ex,ey,ez],w*.043,w*.019,'trunk',6);
      r.crown(ex,ey+h*.044,ez,w*(.47+random()*.15),h*(oak?.35:.42),d*(.51+random()*.13),
        i%3===0?'leafShadow':'leaf',a,random()*.12-.06,random()*.36-.18);
      if(i%2===0)r.crown(ex*.74,ey+h*.16,ez*.74,w*.48,h*(oak?.22:.27),d*.49,'leafSun',a+.52,0,.11);
    }
    r.crown(-w*.07,h*.826,d*.03,w*.48,h*.285,d*.45,'leafSun',.3,-.06,.17);
    r.crown(w*.12,h*.851,-d*.07,w*.37,h*(oak?.17:.24),d*.34,'leaf',.83,.12,-.18);
  }
  function fence(r,p) {
    const w=p.w||4,h=p.h||1.12;
    const count=Math.max(2,Math.ceil(w/2.1)+1);
    for(let i=0;i<count;i++) {
      const x=-w*.5+i*w/(count-1);
      r.box(x,h*.49,0,.13,h*.98,.14,'weatherWood');
      r.cylinder(x,h+.019,0,.09,.025,.10,'woodcut',4,Math.PI/4);
    }
    for(const y of [.32,.76])r.box(0,h*y,.04,w,.12,.11,'weatherWood',0,0,y>.5?.012:-.008);
  }
  function wall(r,p,random) {
    const w=p.w||4,d=p.d||.6,h=p.h||1;
    r.box(0,h*.47,0,w,h*.91,d,'rubble');
    const count=Math.max(2,Math.ceil(w/.72)),sw=w/count;
    for(let i=0;i<count;i++){
      const cap=.10+random()*.09;
      r.box(-w*.5+sw*(i+.5),h*.88+cap*.5,0,sw-.025,cap,d+.085,
        i%4<2?'limestone':'rubble',0,0,(random()-.5)*.085);
    }
  }
  function sacks(r,p,random) {
    const w=p.w||3.3,d=p.d||.78,h=p.h||1.02;
    const layers=Math.max(2,Math.round(h/.27)),n=Math.max(2,Math.round(w/.75)),length=w/n;
    for(let j=0;j<layers;j++) {
      const offset=j%2?.5:0;
      for(let i=0;i<n+(j%2);i++){
        const left=Math.max(0,i-offset),right=Math.min(n,i+1-offset);
        r.sphere(-w*.5+(left+right)*length*.5,h/layers*(j+.52),d*(random()-.5)*.06,
          length*(right-left)*1.08,h/layers*1.18,d*(.88+random()*.13),
          ['sack','sackShade','sackLight'][Math.floor(random()*3)],(random()-.5)*.10);
      }
    }
  }
  function boxCrate(r,x,y,z,w,h,d,type) {
    const paint=type==='ammo'?'fieldPaint':type==='metal'?'truckDark':'weatherWood';
    r.box(x,y+h*.5,z,w,h,d,paint);
    for(const s of [-1,1]){
      const xx=x+s*w*.38;
      r.box(xx,y+h*.50,z,Math.min(.10,w*.13),h+.045,d+.055,type==='metal'?'iron':'joinery');
    }
    r.box(x,y+h*.95,z,w+.032,.075,d+.036,paint);
    if(type==='ammo'){
      for(const sign of [-1,1]) {
        r.box(x,y+h*.55,z+sign*(d*.5+.012),w*.30,h*.19,.025,'stencil');
        r.box(x+w*.125,y+h*.31,z+sign*(d*.5+.015),w*.10,.035,.022,'mustard');
      }
    } else if(type==='metal'){
      for(const sign of [-1,1])
        r.box(x,y+h*.55,z+sign*(d*.5+.018),w*.24,.068,.048,'paleSteel');
    } else {
      const zf=z+d*.5+.03;
      stick(r,x-w*.41,y+h*.18,zf,x+w*.41,y+h*.84,zf,.085,.07,'woodcut');
    }
  }
  function supply(r,p) {
    const w=p.w||1.45,d=p.d||1.0,h=p.h||1.0;
    if(p.kind==='logs'){
      const diam=Math.min(h/2.3,d/2.55);
      for(let row=0;row<2;row++)for(let col=0;col<3-row;col++){
        const yy=diam*(.52+row*.84),zz=(col-(2-row)/2)*diam*.95;
        r.cylinder(0,yy,zz,diam*.49,diam*.51,w*(1-.035*(row+col)),row?'timber':'trunk',8,0,0,halfPi);
        for(const s of [-1,1])
          r.cylinder(s*w*.5*(1-.035*(row+col)),yy,zz,diam*.455,diam*.455,.018,'woodcut',8,0,0,halfPi);
      }
    }else {
      boxCrate(r,0,0,0,w,h,d,p.resource);
    }
  }
  function drums(r,p){
    const w=p.w||1.5,d=p.d||1.1,h=p.h||1.02;
    const count=w/d>1.38?2:1, radius=Math.min(d*.45,w/(count*2.15));
    for(let i=0;i<count;i++){
      const x=(i-(count-1)*.5)*radius*2.12;
      r.cylinder(x,h*.49,0,radius*.94,radius*.95,h*.98,i%2?'truckDark':'oxide',12);
      for(const y of [.05,.30,.70,.97]) r.cylinder(x,h*y,0,radius,radius,.035,'iron',12);
      r.cylinder(x+radius*.46,h*.992,radius*.12,.05,.05,.02,'lead',8);
    }
  }
  function lookout(r,p) {
    const w=p.w||3,d=p.d||3,h=p.h||8, floor=h*.69;
    for(const sx of [-1,1])for(const sz of [-1,1]){
      stick(r,sx*w*.48,.08,sz*d*.48,sx*w*.37,floor+.94,sz*d*.37,.17,.17,'darkWood');
      stick(r,sx*w*.46,.4,sz*d*.46,-sx*w*.375,floor-.20,sz*d*.38,.105,.105,'joinery');
      if(sx===1)stick(r,sx*w*.45,.46,sz*d*.46,sx*w*.375,floor-.2,-sz*d*.38,.115,.115,'joinery');
      r.box(sx*w*.38,(h+floor)*.5-.13,sz*d*.38,.12,h-floor-.15,.13,'weatherWood');
    }
    r.box(0,floor,0,w*.96,.19,d*.96,'weatherWood');
    for(const s of [-1,1]){
      r.box(0,floor+.45,s*d*.404,w*.82,.71,.12,'barnBoard');
      r.box(s*w*.405,floor+.46,0,.12,.74,d*.83,'barnBoard');
    }
    r.box(0,h-.06,0,w*1.08,.16,d*1.1,'slate',0,.065);
    for(const s of [-1,1])stick(r,s*.23,.10,d*.67,s*.23,floor,d*.43,.075,.075,'weatherWood');
    for(let i=1;i<Math.floor(floor/.38);i++)r.box(0,i*.38,d*(.67-(i*.38/floor)*.24),.49,.065,.06,'woodcut');
  }
  function pole(r,p){
    const h=p.h||6.2,w=p.w||1.65;
    r.cylinder(0,h*.5,0,.14,.091,h,'trunk',7);
    r.box(0,h*.91,0,w,.13,.14,'weatherWood');
    for(const s of [-1,1]){
      stick(r,0,h*.8,0,s*w*.36,h*.905,0,.06,.06,'oxide');
      for(const j of [.23,.46]) {
        r.cylinder(s*w*j,h*.934,0,.055,.052,.19,'iron',6);
        r.cylinder(s*w*j,h*.945,0,.083,.072,.09,'creamPaint',6);
      }
    }
  }
  function telegraph(r,poles) {
    for(let i=1;i<poles.length;i++){
      const a=poles[i-1],b=poles[i];
      if(Math.hypot(b.x-a.x,b.z-a.z)>30)continue;
      for(const side of [-1,1]){
        let lx,ly,lz;
        for(let j=0;j<5;j++){
          const t=j*.25,x=(a.x+side*a.w*.46)*(1-t)+(b.x+side*b.w*.46)*t;
          const y=(a.h*(1-t)+b.h*t)*.95-Math.sin(t*Math.PI)*.42;
          const z=a.z*(1-t)+b.z*t;
          if(j)stick(r,lx,ly,lz,x,y,z,.027,.027,'iron');
          lx=x;ly=y;lz=z;
        }
      }
    }
  }
  function iconBoard(r,x,y,z,face,type){
    r.group(x,y,z,face,()=>{
      r.box(0,0,0,.666,.475,.071,'darkWood');
      r.box(0,0,.042,.616,.429,.018,'creamPlate');
      if(type==='oil'){
        r.sphere(0,-.035,.058,.14,.145,.026,'oilBlack');
        r.gable(0,.044,.059,.127,.152,.028,'oilBlack');
      }else if(type==='refinery'){
        r.box(0,-.018,.058,.179,.186,.029,'pumpRed');
        r.box(0,.09,.058,.087,.044,.029,'pumpRed');
        r.box(.089,-.024,.059,.031,.111,.027,'creamPlate');
      }else if(type==='garage'){
        for(const s of [-1,1])r.cylinder(.11*s,-.073,.059,.053,.053,.024,'iron',8,0,halfPi);
        r.box(0,-.014,.061,.315,.075,.03,'truckPaint');
        r.box(.041,.049,.061,.136,.075,.03,'truckPaint');
      }else{
        stick(r,-.116,-.106,.058,.101,.11,.058,.051,.028,'truckDark');
        r.cylinder(.091,.095,.055,.067,.067,.029,'truckDark',8,0,halfPi);
        r.cylinder(.108,.122,.075,.045,.045,.014,'creamPlate',8,0,halfPi);
      }
    });
  }
  function oilwell(r){
    // Compact field pumpjack. The horizontal beam and balance wheel identify a crude site.
    r.box(0,.081,0,2.18,.162,1.65,'concrete');
    for(const side of [-1,1]){
      r.box(0,.196,side*.472,1.95,.10,.097,'iron');
      stick(r,-.208,.251,side*.448,.111,1.97,side*.241,.107,.099,'truckDark');
      stick(r,.536,.251,side*.448,.111,1.97,side*.241,.107,.099,'truckDark');
      stick(r,-.099,1.0,side*.386,.353,1.0,side*.386,.074,.072,'steel');
    }
    r.box(-.025,1.98,0,1.757,.151,.194,'pumpRed',0,0,-.105);
    r.sphere(.762,1.794,0,.338,.516,.231,'pumpRed',0,0,-.21);
    r.box(.816,1.617,0,.080,.207,.202,'iron',0,0,-.11);
    r.cylinder(.83,.923,0,.02,.021,1.22,'paleSteel',6);
    r.cylinder(.83,.324,0,.113,.071,.253,'iron',8);
    r.box(-.608,.532,0,.56,.55,.415,'truckDark');
    for(const s of [-1,1]){
      r.cylinder(-.695,.676,s*.278,.32,.32,.086,'pumpRed',12,0,halfPi);
      r.cylinder(-.695,.676,s*.33,.071,.071,.075,'steel',8,0,halfPi);
      stick(r,-.852,.857,s*.36,-.758,2.041,s*.175,.059,.064,'steel');
    }
    r.cylinder(1.0,1.23,-.68,.032,.025,2.4,'paleSteel',8);
    iconBoard(r,0,1.06,.489,0,'oil');
  }
  function refinery(r){
    r.box(0,.114,0,3.20,.228,2.68,'concrete');
    for(const side of [-1,1])r.box(side*1.32,.26,0,.139,.129,2.25,'iron');
    for(const x of [-.72,.33]){
      const height=x<0?1.74:1.33;
      r.cylinder(x,.30+height*.5,-.29,.486,.486,height,x<0?'truckPaint':'oxide',16);
      r.sphere(x,.30+height,-.29,.966,.297,.966,x<0?'truckPaint':'oxide');
      for(const k of [.10,.52,.93])r.cylinder(x,.30+height*k,-.29,.502,.502,.053,'iron',16);
      r.cylinder(x,.405+height,-.29,.107,.103,.219,'steel',10);
    }
    r.cylinder(.961,1.432,-.682,.17,.15,2.27,'iron',12);
    for(let j=0;j<4;j++)r.cylinder(.961,1.872+.157*j,-.682,.164,.160,.061,'pumpRed',10);
    for(const x of [-.73,.33]){
      r.cylinder(x,.634,.439,.055,.055,1.30,'oxide',8,0,halfPi);
      r.cylinder(x,.827,1.016,.053,.05,.379,'oxide',8);
      r.cylinder(x,1.023,1.016,.122,.122,.044,'pumpRed',10);
    }
    r.box(.952,.548,.59,.42,.574,.582,'truckDark');
    iconBoard(r,.961,.663,.903,0,'refinery');
    for(let i=0;i<4;i++)r.box(.953,.286,.673+i*.102,.55,.047,.04,'steel');
  }
  function workshop(r){
    r.box(0,.098,0,2.88,.196,2.09,'concrete');
    for(const x of [-1.23,1.23])for(const z of [-.852,.851])
      r.box(x,1.235,z,.117,2.26,.141,'weatherWood');
    r.box(0,2.397,0,2.89,.124,2.27,'slate',0,-.078);
    r.box(0,1.155,-.893,2.52,1.90,.096,'barnBoard');
    r.box(0,1.02,-.315,2.36,.158,.78,'weatherWood');
    for(const x of [-1.031,1.031])for(const z of [-.63,-.008])r.box(x,.51,z,.108,.859,.107,'darkWood');
    for(let i=0;i<4;i++) {
      const x=-.815+i*.245;
      r.box(x,1.525,-.821,.043,.296,.046,'steel',0,0,i%2?.21:-.08);
      r.cylinder(x,1.686,-.816,.061,.061,.049,'paleSteel',8,0,halfPi);
    }
    r.box(.608,1.19,-.287,.29,.218,.19,'truckDark');
    r.cylinder(.763,1.195,-.291,.035,.035,.25,'steel',8,0,0,halfPi);
    boxCrate(r,-.752,.195,.53,.679,.41,.47,'metal');
    iconBoard(r,.877,1.843,.881,0,'workshop');
    r.cylinder(-.966,1.982,.524,.091,.073,.23,'lamp',8);
  }
  function fuelCan(r,x,y,z,yaw=0){
    r.group(x,y,z,yaw,()=>{
      r.box(0,.22,0,.26,.44,.17,'metal');
      r.box(0,.46,0,.14,.036,.08,'iron');
      r.box(.05,.22,.092,.036,.31,.015,'mustard',0,0,.32);
    });
  }
  function garage(r){
    const width=4.36,depth=4.03;
    r.box(0,.086,0,width,.172,depth,'concrete');
    for(const s of [-1,1])for(const z of [-1.82,-.21,1.80]){
      r.box(s*2.038,1.25,z,.155,2.40,.159,'darkWood');
      stick(r,s*2.031,1.808,z,s*1.67,2.408,z,.097,.097,'weatherWood');
    }
    r.box(0,1.23,-1.861,4.00,2.22,.106,'barnBoard');
    for(const s of [-1,1]){
      r.box(s*2.086,1.213,-.652,.083,2.17,2.42,'barnBoard');
      r.box(s*2.087,2.363,0,.151,.18,depth,'darkWood');
    }
    r.gable(0,2.581,0,width,.473,depth,'barnBoard');
    pitchedRoof(r,width,depth,2.818,2.345,'slate','lead');
    r.box(0,2.367,1.89,width-.18,.178,.167,'weatherWood');
    for(const x of [-1.2,1.2])r.box(x,.184,0,.121,.037,depth-.33,'mustard');
    for(let i=0;i<2;i++)r.cylinder(-1.743,.371+i*.304,-1.423,.37,.37,.281,'tire',12);
    fuelCan(r,1.54,.18,-1.18,.24);
    iconBoard(r,1.558,2.139,1.892,0,'garage');
  }
  function powderBox(r,p){
    const w=p.w||1.35,d=p.d||.96,h=p.h||.78;
    boxCrate(r,0,0,0,w,h,d,'ammo');
    for(const s of [-1,1]){
      r.box(0,h*.55,s*(d*.5+.023),w*.414,h*.265,.044,'creamPlate');
      for(let i=-1;i<=1;i++)r.box(i*w*.093,h*.55,s*(d*.5+.050),w*.045,h*.19,.023,'pumpRed',0,0,-.27);
    }
  }

  function prop(r,p) {
    const random=rng(seedOf(p));
    r.group(p.x,0,p.z,p.yaw||0,()=>{
      switch(p.kind) {
        case 'house':case 'barn':return home(r,p,random);
        case 'ruin':return ruin(r,p,random);
        case 'tree':case 'pine':case 'bush':return foliage(r,p,random);
        case 'fence':return fence(r,p);
        case 'wall':return wall(r,p,random);
        case 'sandbag':return sacks(r,p,random);
        case 'powder':return powderBox(r,p);
        case 'logs':case 'crate':return p.resource==='gunpowder'||p.resource==='powder'?powderBox(r,p):supply(r,p);
        case 'barrels':return drums(r,p);
        case 'oilwell':return oilwell(r);
        case 'refinery':return refinery(r);
        case 'workshop':return workshop(r);
        case 'garage':return garage(r);
        case 'tower':return lookout(r,p);
        case 'pole':return pole(r,p);
        case 'stone':return r.sphere(0,p.h*.43,0,p.w,p.h,p.d,'basalt',random(),.09,.12);
      }
    });
  }
  function mast(r,o){
    r.group(o.x,0,o.z,0,()=>{
      const ht=11.3,base=.79,cap=.145;
      for (const sx of [-1,1])for(const sz of [-1,1]){
        r.box(sx*base,.13,sz*base,.42,.26,.42,'rubble');
        stick(r,sx*base,.25,sz*base,sx*cap,ht,sz*cap,.098,.098,'aerial');
      }
      const spans=7;
      for(let k=0;k<spans;k++){
        const y1=.36+(ht-.36)*k/spans,y2=.36+(ht-.36)*(k+1)/spans;
        const r1=base+(cap-base)*k/spans,r2=base+(cap-base)*(k+1)/spans;
        for(const side of [-1,1]){
          stick(r,-r1,y1,side*r1,r1,y1,side*r1,.065,.065,'steel');
          stick(r,side*r1,y1,-r1,side*r1,y1,r1,.065,.065,'steel');
          const alt=k%2?-1:1;
          stick(r,-alt*r1,y1,side*r1,alt*r2,y2,side*r2,.065,.065,'aerial');
          stick(r,side*r1,y1,-alt*r1,side*r2,y2,alt*r2,.065,.065,'aerial');
        }
      }
      r.cylinder(0,ht+.73,0,.052,.035,1.55,'paleSteel',8);
      r.box(0,ht+1.25,0,3.4,.07,.065,'aerial');
      for(const s of [-1,1])r.box(s*1.37,ht+1.25,0,.055,.06,1.18,'aerial');
      r.sphere(0,ht+1.55,0,.15,.17,.15,'redLead');
      // Field telephone, two coils and a dispatch lamp below the aerial.
      r.box(0,1.06,.63,1.31,.13,.65,'joinery');
      for(const sx of [-1,1])for(const sz of [-1,1])
        r.box(sx*.53,.53,.63+sz*.23,.085,1.06,.085,'steel');
      r.box(-.11,1.34,.65,.85,.47,.46,'truckDark');
      r.box(-.11,1.36,.888,.75,.36,.033,'iron');
      r.box(-.22,1.41,.912,.31,.12,.025,'stencil');
      for(const sx of [-.35,-.06,.16])r.cylinder(sx,1.27,.918,.049,.049,.045,'paleSteel',8,0,halfPi);
      r.cylinder(.39,1.34,.63,.08,.08,.25,'lamp',8);
      r.cylinder(.39,1.495,.63,.12,.07,.061,'iron',8);
      r.box(0,3.12,-.64,1.14,.62,.065,'canvas');
      for(let k=0;k<3;k++)r.box((k-1)*.15,3.07+k*.045,-.68,.078,.2+k*.09,.025,'fieldPaint');
    });
  }
module.exports = { materials, prop, mast, telegraph };
