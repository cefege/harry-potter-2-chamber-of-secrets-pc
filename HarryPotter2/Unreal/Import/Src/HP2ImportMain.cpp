#include "HP2Import.h"
#include "hp2_rs.h"
#include <iostream>
#include <string>
#include <vector>
using namespace HP2Import;
static bool Parse(int argc,char**argv,Options&o,std::string&error){bool retail=false,archive=false;for(int i=1;i<argc;++i){std::string a=argv[i];auto value=[&](const char*name,std::string&out){if(i+1>=argc){error=std::string("argument ")+name+": expected one argument";return false;}out=argv[++i];return true;};if(a=="--prototype-root"){if(!value("--prototype-root",o.prototypeRoot))return false;}else if(a=="--retail-root"){if(!value("--retail-root",o.retailRoot))return false;retail=true;}else if(a=="--archive"){if(!value("--archive",o.archive))return false;archive=true;}else if(a=="--output"){if(!value("--output",o.output))return false;}else if(a=="--profile"){std::string v;if(!value("--profile",v))return false;if(v=="safe")o.profile=Profile::Safe;else if(v=="full")o.profile=Profile::Full;else if(v=="retail-only")o.profile=Profile::RetailOnly;else{error="argument --profile: invalid choice: '"+v+"' (choose from 'safe', 'full', 'retail-only')";return false;}}else if(a=="--link-mode"){std::string v;if(!value("--link-mode",v))return false;if(v=="auto")o.linkMode=LinkMode::Auto;else if(v=="copy")o.linkMode=LinkMode::Copy;else if(v=="hardlink")o.linkMode=LinkMode::Hardlink;else{error="argument --link-mode: invalid choice: '"+v+"' (choose from 'auto', 'copy', 'hardlink')";return false;}}else if(a=="--check")o.checkOnly=true;else{error="unrecognized arguments: "+a;return false;}}if(retail==archive){error="one of the arguments --retail-root --archive is required";return false;}if(o.output.empty()){error="the following arguments are required: --output";return false;}return true;}
namespace {
void CaptureSink(void* ctx, const uint8_t* text, size_t len) {
    static_cast<std::string*>(ctx)->assign(reinterpret_cast<const char*>(text), len);
}
}
int main(int argc, char** argv) {
    Options options;
    std::string error;
    if (!Parse(argc, argv, options, error)) {
        std::cerr << "hp2_import: " << error << '\n';
        return 2;
    }
    std::string log;
    const bool ok = hp2_rs_import_run(
        reinterpret_cast<const uint8_t*>(options.retailRoot.data()), options.retailRoot.size(),
        reinterpret_cast<const uint8_t*>(options.archive.data()), options.archive.size(),
        reinterpret_cast<const uint8_t*>(options.prototypeRoot.data()), options.prototypeRoot.size(),
        reinterpret_cast<const uint8_t*>(options.output.data()), options.output.size(),
        static_cast<uint32_t>(options.profile),
        static_cast<uint32_t>(options.linkMode),
        options.checkOnly,
        nullptr,
        &CaptureSink,
        &log);
    if (!ok) {
        std::cerr << "hp2_import: " << log << '\n';
        return 2;
    }
    std::cout << log;
    return 0;
}
