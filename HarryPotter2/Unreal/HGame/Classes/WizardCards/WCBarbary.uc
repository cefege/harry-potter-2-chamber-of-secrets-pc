//===============================================================================
//  #93 Heathcote Barbary
//===============================================================================

class  WCBarbary extends bronzecards;

#EXEC TEXTURE IMPORT NAME=WizardCardBarbaryTex0  FILE=..\HGame\Textures\Menu\Folio\Cards\Barbarysmall.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=WizCardBarbaryBigTexture FILE=TEXTURES\menu\Folio\Cards\Barbarybig.bmp GROUP="Icons" FLAGS=2 MIPS=OFF

function PostBeginPlay()
{
	WizardName = "Heathcote Barbary";
	
	Super.PostBeginPlay();
}

defaultproperties
{
	ID=93
    skin(0)=Texture'HProps.skins.WizardCardBarbaryTex0'
	textureBig=Texture'WizCardBarbaryBigTexture'
	strDescriptionId="WizCard_0007"
}
