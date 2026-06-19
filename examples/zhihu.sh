#!/usr/bin/env bash
# 抓取知乎专栏文章。
#
# 用法:
#   ./zhihu.sh <文章URL> <d_c0值> <__zse_ck值>
#
# 示例:
#   ./zhiwu.sh "https://zhuanlan.zhihu.com/p/688930250" \
#       "nCNUCQYE1huPTtZAE6mWvcT_STcM6juvaxQ=|1770998819" \
#       "005_wJ1IMEJmQIwpbyN4vbgZM8LphjmXkkLV..."
#
# 知乎风控要点:
#   - 必须带 d_c0 (设备指纹) + __zse_ck (验证 token) cookie, 否则 403
#   - 正文在初始 HTML 的 <script id="js-initialData"> JSON 里 (SSR),
#     直接解析即可, 不依赖 JS 渲染, 绕过 aginxbrower DOM 不完整的问题
#   - 直连服务器 IP 即可, 不需要住宅代理 (之前误判为 IP 问题, 实为 cookie)
#
# __zse_ck 是动态 token, 会过期 (通常几天), 失效后从浏览器重新导出:
#   F12 -> Application -> Cookies -> .zhihu.com -> __zse_ck
#
# 服务地址可通过 AGINXBROWER_ADDR 环境变量覆盖 (默认本机 8089)。

set -euo pipefail

URL="${1:?用法: $0 <文章URL> <d_c0> <__zse_ck>}"
DC0="${2:?缺少 d_c0}"
ZSECK="${3:?缺少 __zse_ck}"
ADDR="${AGINXBROWER_ADDR:-127.0.0.1:8089}"

# 从 js-initialData 同步提取正文 (不走 async/JS 渲染, 稳定可靠)
SCRIPT='(()=>{
  const s=document.querySelector("script#js-initialData");
  if(!s) return {err:"no initialData (cookie 失效或被风控?)"};
  const data=JSON.parse(s.textContent);
  const arts=data.initialState?.entities?.articles||{};
  const article=Object.values(arts)[0];
  if(!article) return {err:"文章未找到", title:document.title};
  const strip=html=>(html||"").replace(/<[^>]+>/g,"").replace(/&nbsp;/g," ").replace(/&amp;/g,"&").replace(/&lt;/g,"<").replace(/&gt;/g,">").trim();
  return {
    title: article.title,
    author: article.author?.name,
    voteup: article.voteupCount,
    comment: article.commentCount,
    created: article.created,
    bodyLen: strip(article.content).length,
    body: strip(article.content)
  };
})()'

# 构造 JSON 请求体 (用 python 避免 shell 转义地狱)
python3 -c "
import json,sys
req={'url':sys.argv[1],'script':sys.argv[2],'cookies':[ 'd_c0='+sys.argv[3], '__zse_ck='+sys.argv[4] ]}
sys.stdout.write(json.dumps(req,ensure_ascii=False))
" "$URL" "$SCRIPT" "$DC0" "$ZSECK" > /tmp/zhihu_req.json

curl -s -X POST "http://$ADDR/eval" -H "Content-Type: application/json" -d @/tmp/zhihu_req.json --max-time 60 \
  | python3 -c "
import json,sys
d=json.load(sys.stdin)
r=d.get('result')
if not isinstance(r,dict) or r.get('err'):
    print('抓取失败:', json.dumps(d,ensure_ascii=False)[:300]); sys.exit(1)
import datetime
ts=datetime.datetime.fromtimestamp(r.get('created',0)).strftime('%Y-%m-%d') if r.get('created') else '?'
print('='*60)
print('标题:', r.get('title'))
print('作者:', r.get('author'))
print('发布:', ts, ' | 赞:', r.get('voteup'), ' | 评论:', r.get('comment'))
print('正文:', r.get('bodyLen'), '字符')
print('='*60)
print(r.get('body',''))
"
