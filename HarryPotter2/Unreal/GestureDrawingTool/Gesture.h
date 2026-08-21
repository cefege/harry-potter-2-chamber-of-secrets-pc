//***********************************************************************
//
//  Gesture.h
//
//***********************************************************************

#define MAX_POINTS	128

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
	short		m_nSectorsArray[32];

	CGesture();
	void ConvertFromLineArray(CObArray *lineArray);
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
	CLine	*m_MidwayLine;
	CLine	*m_SegLineA;
	CLine	*m_SegLineB;
	CLine	*m_SegLineC;
	CLine	*m_SegLineD;

	CGesture m_Gesture;
	bool	m_bReflect;
	char	m_szFilename[128];

    void InvertLine (CDC*, CPoint, CPoint);
    void DeleteAllLines ();

public:
    CMainWindow ();
    ~CMainWindow ();

	void SaveFile();
	void ConvertGestureFromLineArray();
	void ConvertGestureToLineArray();
	short PlaceSegment(CFloatPoint *);

protected:
    afx_msg void OnPaint ();
    afx_msg void OnFileNew ();
    afx_msg void OnUpdateFileNewUI (CCmdUI*);
    afx_msg void OnUpdateFileOpenUI(CCmdUI*);
	afx_msg void OnUpdateReflectUI(CCmdUI*);
	afx_msg void OnFileSave ();
	afx_msg void OnFileSaveAs ();
	afx_msg void OnFileOpen ();
	afx_msg void OnReflect ();
	afx_msg void OnFlip ();	
	afx_msg void OnCenter ();
	afx_msg void OnEndGesture ();
    afx_msg void OnFileExit ();
    afx_msg void OnLButtonDown (UINT, CPoint);
    afx_msg void OnMouseMove (UINT, CPoint);
    afx_msg void OnLButtonUp (UINT, CPoint);
    afx_msg void OnContextMenu (CWnd*, CPoint);
    afx_msg void OnMeasureItem (int, LPMEASUREITEMSTRUCT);
    afx_msg void OnDrawItem (int, LPDRAWITEMSTRUCT);
    
    DECLARE_MESSAGE_MAP ()
};
