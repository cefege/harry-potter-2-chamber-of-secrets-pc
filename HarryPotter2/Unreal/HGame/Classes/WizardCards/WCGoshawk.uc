//===============================================================================
// #46 Miranda Goshawk
//===============================================================================

class  WCGoshawk extends Bronzecards;

#EXEC TEXTURE IMPORT NAME=WizardCardGoshawkTex0  FILE=..\HGame\Textures\Menu\Folio\Cards\Goshawksmall.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=WizCardGoshawkBigTexture FILE=TEXTURES\menu\Folio\Cards\Goshawkbig.bmp GROUP="Icons" FLAGS=2 MIPS=OFF

function PostBeginPlay()
{
	WizardName = "Miranda Goshawk";
	
	Super.PostBeginPlay();
}

defaultproperties
{
	ID=46
	bVendorsCanSell=true
	strVendorOwnedAfterGState="GSTATE090"
    skin(0)=Texture'HProps.skins.WizardCardGoshawkTex0'
	textureBig=Texture'WizCardGoshawkBigTexture'
	strDescriptionId="WizCard_0084"
}
