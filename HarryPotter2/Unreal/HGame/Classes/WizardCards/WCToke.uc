//===============================================================================
//  #28 Tilly Toke
//===============================================================================

class  WCToke extends bronzecards;

#EXEC TEXTURE IMPORT NAME=WizardCardTokeTex0  FILE=..\HGame\Textures\Menu\Folio\Cards\Tokesmall.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=WizCardTokeBigTexture FILE=TEXTURES\menu\Folio\Cards\Tokebig.bmp GROUP="Icons" FLAGS=2 MIPS=OFF


function PostBeginPlay()
{
	WizardName = "Tilly Toke";
	
	Super.PostBeginPlay();
}

defaultproperties
{
	ID=28
    strVendorOwnedAfterGState="GSTATE090"
    skin(0)=Texture'HProps.skins.WizardCardTokeTex0'
	textureBig=Texture'WizCardTokeBigTexture'
	strDescriptionId="WizCard_0024"
}
