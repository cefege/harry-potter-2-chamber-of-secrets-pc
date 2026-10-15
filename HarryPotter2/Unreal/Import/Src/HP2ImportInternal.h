#pragma once
#include "HP2Import.h"
#include "HP2PortableSha256.h"
#include <utf8proc.h>
#include <algorithm>
#include <array>
#include <chrono>
#include <cctype>
#include <filesystem>
#include <fstream>
#include <iomanip>
#include <map>
#include <set>
#include <sstream>
#include <system_error>
#include <vector>

namespace HP2Import::Detail {
namespace fs = std::filesystem;
constexpr std::size_t ChunkSize=1024*1024;
struct Asset { std::string source, sourcePath, path, classification, hash, link; fs::path disk; std::string content; bool generated=false; std::uintmax_t size=0; std::vector<std::string> sources; };
struct Failure { std::string message; };
[[noreturn]] inline void Fail(std::string s){throw Failure{std::move(s)};}
inline std::string Lower(std::string s){
    bool ascii=true;
    for(char& c:s){
        const auto byte=static_cast<unsigned char>(c);
        if(byte>=0x80){ascii=false;continue;}
        if(c>='A'&&c<='Z')c=static_cast<char>(c+('a'-'A'));
    }
    if(ascii)return s;
    utf8proc_uint8_t* folded=nullptr;
    const auto size=utf8proc_map(reinterpret_cast<const utf8proc_uint8_t*>(s.data()),
        static_cast<utf8proc_ssize_t>(s.size()),&folded,UTF8PROC_CASEFOLD);
    if(size<0)Fail("invalid UTF-8 for case-insensitive comparison: "+s);
    std::string result;
    if(size>0)result.assign(reinterpret_cast<const char*>(folded),static_cast<std::size_t>(size));
    utf8proc_free(folded);
    return result;
}
inline std::string Utf8(const fs::path& p){auto u=p.u8string();return std::string(reinterpret_cast<const char*>(u.data()),u.size());}
inline fs::path Path(const std::string& s){return fs::u8path(s);}
inline std::string Hex(const unsigned char* b,std::size_t n){static constexpr char h[]="0123456789abcdef";std::string s;s.reserve(n*2);for(std::size_t i=0;i<n;++i){s+=h[b[i]>>4];s+=h[b[i]&15];}return s;}
inline std::pair<std::uintmax_t,std::string> Hash(const fs::path& p){std::ifstream f(p,std::ios::binary);if(!f)Fail("cannot hash "+Utf8(p)+": cannot open file");CC_SHA256_CTX c;CC_SHA256_Init(&c);static thread_local std::vector<char>b(ChunkSize);std::uintmax_t n=0;while(f){f.read(b.data(),static_cast<std::streamsize>(b.size()));auto got=f.gcount();if(got>0){n+=static_cast<std::uintmax_t>(got);CC_SHA256_Update(&c,b.data(),static_cast<CC_LONG>(got));}}if(f.bad())Fail("cannot hash "+Utf8(p)+": read error");unsigned char d[32];CC_SHA256_Final(d,&c);return {n,Hex(d,32)};}
inline bool Starts(const std::string&s,const std::string&p){return s.size()>=p.size()&&s.compare(0,p.size(),p)==0;}
inline bool Ends(const std::string&s,const std::string&p){return s.size()>=p.size()&&s.compare(s.size()-p.size(),p.size(),p)==0;}
inline bool Debris(const fs::path&p){auto n=Lower(Utf8(p.filename())),e=Lower(Utf8(p.extension()));static const std::set<std::string>x={".ds_store","desktop.ini","mssccprj.scc","thumbs.db","vssver.scc","ws_ftp.log"};return x.find(n)!=x.end()||e==".scc"||Starts(n,"unins")||Ends(n,".bak")||Ends(n,".old")||Ends(n,".orig")||Ends(n,".tmp")||Ends(n,"~");}
inline bool Prohibited(const std::string&p){auto l=Lower(p);auto ext=Lower(Utf8(Path(p).extension()));static const std::set<std::string>x={".bat",".cmd",".com",".dll",".drv",".exe",".lib",".ocx",".sys"};if(x.find(ext)!=x.end()||Debris(Path(p)))return true;std::stringstream ss(l);std::string a;while(std::getline(ss,a,'/'))if(a==".git"||a==".svn"||a=="cvs"||a=="__pycache__")return true;return false;}
inline Asset MakeAsset(const std::string&source,const fs::path&root,const fs::path&file,std::string dest){if(fs::is_symlink(file)||!fs::is_regular_file(file))Fail("input asset is not a real regular file: "+Utf8(file));auto rel=Utf8(file.lexically_relative(root).generic_u8string());if(rel.empty()||Starts(rel,"../"))Fail("input asset escapes "+source+" root: "+Utf8(file));if(Prohibited(dest))Fail("prohibited asset selected for overlay: "+dest);auto [size,hash]=Hash(file);Asset a;a.source=source;a.sourcePath=rel;a.path=std::move(dest);a.hash=std::move(hash);a.disk=file;a.size=size;return a;}
fs::path Extract(const fs::path&,const fs::path&);
inline fs::path Child(const fs::path&d,const std::string&w,bool required=true){std::vector<fs::path>m;std::error_code ec;for(fs::directory_iterator i(d,ec),end;i!=end&&!ec;i.increment(ec))if(Lower(Utf8(i->path().filename()))==Lower(w))m.push_back(i->path());if(ec)Fail("cannot inspect "+Utf8(d)+": "+ec.message());if(m.size()>1)Fail("ambiguous case-insensitive component '"+w+"' under "+Utf8(d));if(m.empty()){if(required)Fail("missing "+w+" directory under "+Utf8(d));return {}; }auto st=fs::symlink_status(m[0],ec);if(ec||fs::is_symlink(st)||!fs::is_directory(st))Fail("expected real directory for "+w+": "+Utf8(m[0]));return m[0];}
inline fs::path ChildFile(const fs::path&d,const std::string&w){std::vector<fs::path>m;for(auto&i:fs::directory_iterator(d))if(Lower(Utf8(i.path().filename()))==Lower(w))m.push_back(i.path());if(m.size()!=1)Fail("expected exactly one "+w+" under "+Utf8(d)+", found "+std::to_string(m.size()));if(fs::is_symlink(m[0])||!fs::is_regular_file(m[0]))Fail("expected real config file: "+Utf8(m[0]));return m[0];}
std::vector<Asset> Plan(const Options&,const fs::path&,const fs::path&);
std::string Manifest(const std::vector<Asset>&,const std::string&);
std::string Checksums(const std::vector<Asset>&);
}
