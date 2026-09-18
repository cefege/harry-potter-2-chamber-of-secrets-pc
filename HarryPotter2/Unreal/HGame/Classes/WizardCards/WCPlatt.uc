//===============================================================================
//  #95 Yardley Platt
//===============================================================================

class  WCPlatt extends bronzecards;

#EXEC TEXTURE IMPORT NAME=WizardCardPlattTex0  FILE=..\HGame\Textures\Menu\Folio\Cards\Plattsmall.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=WizCardPlattBigTexture FILE=TEXTURES\menu\Folio\Cards\Plattbig.bmp GROUP="Icons" FLAGS=2 MIPS=OFF

function PostBeginPlay()
{
	WizardName = "Yardley Platt";
	
	Super.PostBeginPlay();
}

defaultproperties
{
	ID=95
	bVendorsCanSell=true
	strVendorOwnedAfterGState="GSTATE120"
    skin(0)=Texture'HProps.skins.WizardCardPlattTex0'
	textureBig=Texture'WizCardPlattBigTexture'
	strDescriptionId="WizCard_0014"
}
