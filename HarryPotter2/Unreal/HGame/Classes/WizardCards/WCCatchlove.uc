//===============================================================================
// #53 Greta Catchlove
//===============================================================================

class  WCCatchlove extends Bronzecards;

#EXEC TEXTURE IMPORT NAME=WizardCardCatchloveTex0  FILE=..\HGame\Textures\Menu\Folio\Cards\Catchlovesmall.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=WizCardCatchloveBigTexture FILE=TEXTURES\menu\Folio\Cards\Catchlovebig.bmp GROUP="Icons" FLAGS=2 MIPS=OFF

function PostBeginPlay()
{
	WizardName = "Greta Catchlove";
	
	Super.PostBeginPlay();
}

defaultproperties
{
	ID=53
    skin(0)=Texture'HProps.skins.WizardCardCatchloveTex0'
	textureBig=Texture'WizCardCatchloveBigTexture'
	strDescriptionId="WizCard_0089"
}
