//===============================================================================
// #03 Elfrida Clagg
//===============================================================================

class  WCClagg extends Silvercards;

#EXEC TEXTURE IMPORT NAME=WizardCardClaggTex0  FILE=..\HGame\Textures\Menu\Folio\Cards\Claggsmall.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=WizCardClaggBigTexture FILE=TEXTURES\menu\Folio\Cards\Claggbig.bmp GROUP="Icons" FLAGS=2 MIPS=OFF

function PostBeginPlay()
{
	WizardName = "Elfrida Clagg";
	
	Super.PostBeginPlay();
}

defaultproperties
{
	ID=3
	bVendorsCanSell=true
	strVendorOwnedAfterGState="GSTATE120"
    skin(0)=Texture'HProps.skins.WizardCardClaggTex0'
	textureBig=Texture'WizCardClaggBigTexture'
	strDescriptionId="WizCard_0041"
}
