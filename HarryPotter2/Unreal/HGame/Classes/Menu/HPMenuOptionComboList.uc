class HPMenuOptionComboList extends UWindowComboList;

#EXEC TEXTURE IMPORT NAME=FEComboListLarge	 FILE=TEXTURES\Menu\Options\largepopup.bmp GROUP="Icons" FLAGS=2 MIPS=OFF
#EXEC TEXTURE IMPORT NAME=FEComboListSmall	 FILE=TEXTURES\Menu\Options\smallpopup.bmp GROUP="Icons" FLAGS=2 MIPS=OFF
#EXEC TEXTURE IMPORT NAME=FEComboListBox	 FILE=TEXTURES\Menu\Options\box.bmp GROUP="Icons" FLAGS=2 MIPS=OFF

var Texture bgImage;

function Created ()
{
	Super.Created ();
}

function BeforePaint(Canvas C, float X, float Y)
{
	local float W, H, MaxWidth;
	local int Count;
	local UWindowComboListItem I;
	local float ListX, ListY;
	local float ExtraWidth;
	
	C.Font = Root.Fonts[F_Normal];
	C.SetPos(0, 0);

	MaxWidth = 147;
	ExtraWidth = ((HBorder + TextBorder) * 2);

	Count = Items.Count();

	if (Count > 3)
	{
		WinHeight = 89;
		bgImage = Texture'FEComboListLarge';
	}
	else
	{
		WinHeight = 61;
		bgImage = Texture'FEComboListSmall';
	}

	ItemHeight = (WinHeight-7)/Count;

	VertSB.Pos = 0;

	for( I = UWindowComboListItem(Items.Next);I != None; I = UWindowComboListItem(I.Next) )
	{
		TextSize(C, RemoveAmpersand(I.Value), W, H);
		if(W + ExtraWidth > MaxWidth)
			MaxWidth = W + ExtraWidth;
	}

	WinWidth = MaxWidth;

	ListX = Owner.EditAreaDrawX;
	ListY = Owner.Button.WinTop + Owner.Button.WinHeight;

	VertSB.HideWindow(); // no scrollbar

	Owner.WindowToGlobal(ListX, ListY, WinLeft, WinTop);
}


function DrawMenuBackground(Canvas C)
{
	DrawClippedTexture(C, 0, 0, bgImage);
}




function ComboList_DrawItem(UWindowComboList Combo, Canvas C, float X, float Y, float W, float H, string Text, bool bSelected)
{
	C.DrawColor.R = 255;
	C.DrawColor.G = 255;
	C.DrawColor.B = 255;
	
	if(bSelected)
	{
		Combo.DrawStretchedTexture(C, X+2, Y, W-5, H, Texture'FEComboListBox');
		C.DrawColor.R = 0;
		C.DrawColor.G = 0;
		C.DrawColor.B = 0;
	}
	else
	{
		C.DrawColor.R = 255;
		C.DrawColor.G = 255;
		C.DrawColor.B = 255;
	}

	Combo.ClipText(C, X + Combo.TextBorder + 2, Y + 5, Text);
}


function DrawItem(Canvas C, UWindowList Item, float X, float Y, float W, float H)
{
	ComboList_DrawItem(Self, C, X, Y, W, H, UWindowComboListItem(Item).Value, Selected == Item);
}
