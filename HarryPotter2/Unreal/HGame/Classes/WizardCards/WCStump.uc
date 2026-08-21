//===============================================================================
//  #4 Grogan Stump
//===============================================================================

class  WCStump extends bronzecards;

#EXEC TEXTURE IMPORT NAME=WizardCardStumpTex0  FILE=..\HGame\Textures\Menu\Folio\Cards\Stumpsmall.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=WizCardStumpBigTexture FILE=TEXTURES\menu\Folio\Cards\Stumpbig.bmp GROUP="Icons" FLAGS=2 MIPS=OFF


function PostBeginPlay()
{
	WizardName = "Grogan Stump";
	
	Super.PostBeginPlay();
}

defaultproperties
{
	ID=4
	bVendorsCanSell=true
	strVendorOwnedAfterGState="GSTATE090"
    skin(0)=Texture'HProps.skins.WizardCardStumpTex0'
	textureBig=Texture'WizCardStumpBigTexture'
	strDescriptionId="WizCard_0042"
}
