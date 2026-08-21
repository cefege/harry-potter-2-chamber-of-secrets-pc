// CutCompiler.cpp : Defines the entry point for the console application.
//

#include "stdafx.h"
#include "CutCompiler.h"

#ifdef _DEBUG
#define new DEBUG_NEW
#undef THIS_FILE
static char THIS_FILE[] = __FILE__;
#endif

/////////////////////////////////////////////////////////////////////////////
// The one and only application object

CWinApp theApp;

using namespace std;

void Compile(CString name)
{
CString outName;
char buf[4096];

int curThread=0;
int curLine=0;
bool bFirstThreadFound=false;	//used if there is no inital thread declared.




	outName=name.Left(name.Find("."))+".int";

	cout << (LPCTSTR) name <<"->" <<(LPCTSTR) outName << endl;

	FILE *fi=fopen("..\\CutScenes\\"+name,"rt");
	if(fi==NULL)
	{
		printf("Error: Can't open input file:%s\n",name);
		return;
	}
	FILE *fo=fopen("..\\system\\CutScenes\\"+outName,"wb");
	if(fo==NULL)
	{
		printf("Error: Can't open output file:%s\n",name);
		return;
	}

	while(!feof(fi))
	{
		CString tstr;

		sprintf(buf,"");	//duplicate last line bug fix. 

		fgets(buf,4096,fi);
		tstr=buf;
		if(tstr.Find("//")>-1)
			tstr=tstr.Left(tstr.Find("//"));

		if(tstr.Find(";")>-1)
			tstr=tstr.Left(tstr.Find(";"));

		tstr.TrimLeft(" ");
		tstr.TrimRight(" \n");
		if(tstr=="")
			continue;

		if(tstr.Find("]")>-1)
		{
			curLine=0;
			fprintf(fo,"[Thread_%d]\n",curThread++);
			bFirstThreadFound=true;
		}
		else
		{
			if(bFirstThreadFound==false)
			{	//if no initial thread make one.
				curLine=0;
				fprintf(fo,"[Thread_%d]\n",curThread++);
				bFirstThreadFound=true;
			}
			fprintf(fo,"line_%d=%s\n",curLine++,tstr);
		}
			

	}

	
	fclose(fi);
	fclose(fo);
/*	
	CFile fi,fo;
		fi.Open(finder.GetFilePath(),"rt",CFile::modeRead);
		fo.Open("CutScenes\\"+finder.GetFileName(),CFile::modeCreate|CFile::modeWrite);
*/
}

int _tmain(int argc, TCHAR* argv[], TCHAR* envp[])
{
	int nRetCode = 0;

	// initialize MFC and print and error on failure
	if (!AfxWinInit(::GetModuleHandle(NULL), NULL, ::GetCommandLine(), 0))
	{
		// TODO: change error code to suit your needs
		cerr << _T("Fatal Error: MFC initialization failed") << endl;
		nRetCode = 1;
	}
//		cout << (LPCTSTR)strHello << endl;

	CFileFind finder;
	BOOL bWorking = finder.FindFile("..\\CutScenes\\*.txt");
	while (bWorking)
	{
		bWorking = finder.FindNextFile();
//		cout << (LPCTSTR) finder.GetFileName() << endl;
		Compile(finder.GetFileName());
		
	
	}



	return nRetCode;
}


