class HPMenuOptionCheckBox extends UWindowCheckbox;

#EXEC TEXTURE IMPORT NAME=FEOptionTickUncheckedTex		FILE=TEXTURES\Menu\Options\checkboxup.bmp GROUP="Icons" FLAGS=2 MIPS=OFF
#EXEC TEXTURE IMPORT NAME=FEOptionTickUncheckedOverTex	FILE=TEXTURES\Menu\Options\checkboxover.bmp GROUP="Icons" FLAGS=2 MIPS=OFF
#EXEC TEXTURE IMPORT NAME=FEOptionTickCheckedTex		FILE=TEXTURES\Menu\Options\checkboxuptick.bmp GROUP="Icons" FLAGS=2 MIPS=OFF
#EXEC TEXTURE IMPORT NAME=FEOptionTickCheckedOverTex	FILE=TEXTURES\Menu\Options\checkboxovertick.bmp GROUP="Icons" FLAGS=2 MIPS=OFF

function Checkbox_SetupSizes(UWindowCheckbox W, Canvas C)
{
	local float TW, TH;

	W.TextSize(C, W.Text, TW, TH);

	W.WinHeight = Max(TH+1, 16);

	W.ImageX = 0;
	W.TextX = 13 + 4;

	W.ImageY = (W.WinHeight - 12) / 2;
	W.TextY = (W.WinHeight - TH) / 2;

	if(W.bChecked) 
	{
		W.UpTexture = Texture'FEOptionTickCheckedTex';
		W.DownTexture = Texture'FEOptionTickCheckedOverTex';
		W.OverTexture = Texture'FEOptionTickCheckedOverTex';
		W.DisabledTexture = None;
	}
	else 
	{
		W.UpTexture = Texture'FEOptionTickUncheckedTex';
		W.DownTexture = Texture'FEOptionTickUncheckedOverTex';
		W.OverTexture = Texture'FEOptionTickUncheckedOverTex';
		W.DisabledTexture = None;
	}
}


function BeforePaint(Canvas C, float X, float Y)
{
	Checkbox_SetupSizes(Self, C);
//	Super.BeforePaint(C, X, Y);
}


function Paint(Canvas C, float X, float Y)
{
	Super.Paint(C,X,Y);
}