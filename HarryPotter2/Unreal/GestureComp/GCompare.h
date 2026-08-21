//***********************************************************************
//
//  GCompare.h
//
//***********************************************************************

#define MAX_POINTS		128
#define SEG_SECTIONS	32

class CLine : public CObject
{
private:
    CPoint m_ptFrom;
    CPoint m_ptTo;

public:
    CLine (CPoint, CPoint);
    virtual void Draw (CDC*, COLORREF Color);
	CPoint GetFromPoint() { return m_ptFrom;}
	CPoint GetToPoint() { return m_ptTo;}
	void ReflectAboutMiddle();
	void AdjustX(long nAdjustmentValue);
};


class CDrawnPoint: public CObject
{
private:
	CPoint m_Point;

public:
	CDrawnPoint(CPoint);
    virtual void Draw (CDC*, COLORREF Color);
	CPoint GetPoint() {return m_Point;}
};


class CMyApp : public CWinApp
{
public:
    virtual BOOL InitInstance ();
};

class CFloatPoint
{
public:
	CFloatPoint(float X,float Y);
	float m_fXVal;
	float m_fYVal;
};

class CGesture
{
private:
	CFloatPoint	*m_Points[MAX_POINTS];
	short		m_nNumPoints;

public:
	short m_nSegments[SEG_SECTIONS];

	CGesture();
	void ConvertToLineArray(CObArray *lineArray);
	void AddPoint(CFloatPoint *lpPoint);
	CFloatPoint *GetPoint(short nIndex) { return m_Points[nIndex]; }
	short GetNumPoints() { return m_nNumPoints;}
	void ClearGesture();
};


class CMainWindow : public CFrameWnd
{
private:
    CPoint m_ptFrom;
    CPoint m_ptTo;
    CObArray m_lineArray;
	CObArray m_DrawnPointsArray;
	UINT	m_nLevel;
	CLine	*m_MidwayLine;
	CGesture m_Gesture;
	bool	m_bReflect;
	char	m_szFilename[128];

    void InvertLine (CDC*, CPoint, CPoint);
    void DeleteAllLines ();
    void DeleteAllPoints ();

	void CompareGesture ();

public:
    CMainWindow ();
    ~CMainWindow ();

	void SaveFile();
	void ConvertGestureFromLineArray();
	void ConvertGestureToLineArray();

protected:
    afx_msg void OnPaint ();
    afx_msg void OnUpdateUI (CCmdUI*);
	afx_msg void OnFileOpen ();
	afx_msg	void OnFileClose ();
	afx_msg void OnFileClear ();
    afx_msg void OnFileExit ();
	afx_msg void OnLevel ();
    afx_msg void OnLButtonDown (UINT, CPoint);
    afx_msg void OnMouseMove (UINT, CPoint);
    afx_msg void OnLButtonUp (UINT, CPoint);
    afx_msg void OnMeasureItem (int, LPMEASUREITEMSTRUCT);
    afx_msg void OnDrawItem (int, LPDRAWITEMSTRUCT);
	afx_msg void OnLevel (UINT nID);
	afx_msg void OnUpdateLevel (CCmdUI* pCmdUI);
    
    DECLARE_MESSAGE_MAP ()
};
