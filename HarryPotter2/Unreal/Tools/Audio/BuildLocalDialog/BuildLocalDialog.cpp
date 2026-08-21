// BuildLocalDialog.cpp : Defines the entry point for the console application.
//

#include "stdafx.h"
#include "BuildLocalDialog.h"
#include "Shlwapi.h"

#ifdef _DEBUG
#define new DEBUG_NEW
#undef THIS_FILE
static char THIS_FILE[] = __FILE__;
#endif

/////////////////////////////////////////////////////////////////////////////
// The one and only application object

CWinApp theApp;

using namespace std;

int _tmain(int argc, TCHAR* argv[], TCHAR* envp[])
{
	int nRetCode = 0;
	TCHAR	acMsg[8192];

	// initialize MFC and print and error on failure
	if (!AfxWinInit(::GetModuleHandle(NULL), NULL, ::GetCommandLine(), 0))
	{
		// TODO: change error code to suit your needs
		cerr << _T("Fatal Error: MFC initialization failed") << endl;
		nRetCode = 1;
	}
	else
	{
		// TODO: code your application's behavior here.
		if (argc != 4)
		{
			cerr << TEXT("usage:  BuildLocalDialog SourceDir DestDir LangID\n");
			return 1;
		}

		//	Validate source directory
		//
		if (!PathIsDirectory(argv[1]))
		{
			_stprintf(acMsg, TEXT("Source directory %s does not exist\n"), argv[1]);
			cerr << acMsg;

			return 2;
		}

		//	Validate target directory
		//
		if (!PathIsDirectory(argv[2]))
		{
			_stprintf(acMsg, TEXT("Target directory %s does not exist\n"), argv[2]);
			cerr << acMsg;

			return 3;
		}

		//	Prepare source directory by adding the name of the package to the target path, 
		//	e.g. ...\AllDialogfre for french
		//
		TCHAR	acTargetPath[8192];
		TCHAR	acCmdLine[8192];

		//	Add the name of the package to the target path, e.g. ...\AllDialogfre for french
		//
		_tcscpy(acTargetPath, argv[2]);
		PathAppend(acTargetPath, "AllDialog");
		_tcscat(acTargetPath, argv[3]);

		//	Build the UCC command line
		//
		_stprintf(acCmdLine, TEXT("ucc pkg import sound \"%s\" \"%s\" compress lipsync"), acTargetPath, argv[1]);

		//	Launch UCC to do the package creation
		//
		STARTUPINFO			si;
		PROCESS_INFORMATION	pi;

		memset(&si, 0, sizeof(STARTUPINFO));
		memset(&pi, 0, sizeof(PROCESS_INFORMATION));
		si.cb = sizeof(STARTUPINFO);

		if (!CreateProcess(NULL, acCmdLine, NULL, NULL, FALSE, 0, NULL, NULL, &si, &pi))
		{
			DWORD dwErr = GetLastError();
			cerr << TEXT("Failed to launch package creation tool\n");

			return 4;
		}

		//	Wait until UCC is finished
		//
		DWORD	dwRet;
		BOOL	bSuccess;

		do
		{
			bSuccess = GetExitCodeProcess(pi.hProcess, &dwRet);
			if (!bSuccess)
			{
				cerr << TEXT("Process exit code failure\n");
			}

			if (dwRet == STILL_ACTIVE)
				Sleep(300);
		}	while (dwRet == STILL_ACTIVE);

		//	Rename the newly created sound package (e.g. AllDialogfre.uax) 
		//	to its correct name (e.g. AllDialog.uax_fre)
		//
		TCHAR	acNewPath[8192];

		_tcscpy(acNewPath, argv[2]);
		PathAppend(acNewPath, "AllDialog.");
		_tcscat(acNewPath, argv[3]);
		_tcscat(acNewPath, "_uax");

		_tcscat(acTargetPath, ".uax");
		_tremove(acNewPath);
		_trename(acTargetPath, acNewPath);

		_stprintf(acMsg, TEXT("Renaming %s to %s\n"), PathFindFileName(acTargetPath), PathFindFileName(acNewPath));
		cout << acMsg;

		cout << TEXT("done\n");
	}

	return nRetCode;
}


