//===============================================================================
// #7 Hesper Starkey
//===============================================================================

class  WCStarkey extends Bronzecards;

#EXEC TEXTURE IMPORT NAME=WizardCardStarkeyTex0  FILE=..\HGame\Textures\Menu\Folio\Cards\Starkeysmall.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=WizCardStarkeyBigTexture FILE=TEXTURES\menu\Folio\Cards\Starkeybig.bmp GROUP="Icons" FLAGS=2 MIPS=OFF

function PostBeginPlay()
{
	WizardName = "Hesper Starkey";
	
	Super.PostBeginPlay();
}

defaultproperties
{
	ID=7
	bVendorsCanSell=true
	strVendorOwnedAfterGState="GSTATE000"
    skin(0)=Texture'HProps.skins.WizardCardStarkeyTex0'
	textureBig=Texture'WizCardStarkeyBigTexture'
	strDescriptionId="WizCard_0045"
}
