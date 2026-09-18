//===============================================================================
//  #80 Beatrix Bloxham
//===============================================================================

class  WCBloxam extends bronzecards;

#EXEC TEXTURE IMPORT NAME=WizardCardBloxamTex0  FILE=..\HGame\Textures\Menu\Folio\Cards\Bloxamsmall.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=WizCardBloxamBigTexture FILE=TEXTURES\menu\Folio\Cards\Bloxambig.bmp GROUP="Icons" FLAGS=2 MIPS=OFF

function PostBeginPlay()
{
	WizardName = "Beatrix Bloxham";
	
	Super.PostBeginPlay();
}

defaultproperties
{
	ID=80
	bVendorsCanSell=true
	strVendorOwnedAfterGState="GSTATE110"
    skin(0)=Texture'HProps.skins.WizardCardBloxamTex0'
	textureBig=Texture'WizCardBloxamBigTexture'
	strDescriptionId="WizCard_0074"
}
